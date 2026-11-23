// FerrousPanel Pure-Rust UI — Infrastructure Views (Web Engine, PHP/Software, Cron/Backup/DNS, Mail, Edge/Polyglot)
import {
  api, esc, fmtTime, statusClass, toast, openModal, closeModal, confirmAction, openJobDrawer,
  capBannerHtml, capDisabledAttr, isCapUnavailable, withBusy,
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
        <p class="page-desc">Chuyển đổi nguyên tử giữa Nginx và FerrousX thuần Rust trên cổng 80/443 với kiểm chứng cấu hình và tự động rollback.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="ws-validate-nginx" ${capDisabledAttr('nginx')}>Kiểm tra Nginx (-t)</button>
        <button type="button" class="btn ${activeEngine === 'nginx' ? '' : 'btn-primary'}" id="ws-switch-nginx" ${activeEngine === 'nginx' || isCapUnavailable('nginx') ? 'disabled' : ''}>Chuyển sang Nginx</button>
        <button type="button" class="btn ${activeEngine === 'ferrousx' ? '' : 'btn-primary'}" id="ws-switch-ferrousx" ${activeEngine === 'ferrousx' || isCapUnavailable('ferrousx') ? 'disabled' : ''}>Chuyển sang FerrousX</button>
      </div>
    </div>

    ${capBannerHtml(['nginx', 'ferrousx'])}

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
          <h2 class="surface-title">FerrousX (Pure-Rust HTTP/TLS Engine)</h2>
          <span class="${statusClass(ws?.ferrousx?.active ? 'active' : 'stopped')}">${ws?.ferrousx?.active ? 'Đang chạy' : 'Đã dừng'}</span>
        </div>
        <div class="meta-line" style="margin-bottom:10px">
          <span>Đã cài: <strong>${ws?.ferrousx?.installed ? 'Có' : 'Chưa'}</strong></span>
          <span>·</span>
          <span>Phiên bản: <span class="mono">${esc(ws?.ferrousx?.version || '—')}</span></span>
          <span>·</span>
          <span>IPC: <span class="mono">${esc(fxIpc?.connected ? 'Connected' : 'Standby')}</span></span>
          ${fxIpc?.rateLimited !== undefined ? `<span>·</span><span>Rate limited: <strong class="mono">${esc(fxIpc.rateLimited)}</strong></span>` : ''}
        </div>
        <div class="row">
          ${fxSync ? `<button type="button" class="btn btn-sm" id="ws-sync-fx" ${capDisabledAttr('ferrousx')}>Đồng bộ FerrousX</button>` : ''}
          <button type="button" class="btn btn-sm" id="ws-fx-reload" ${capDisabledAttr('ferrousx')}>IPC Reload Vhosts</button>
        </div>
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

  function doSwitch(target, btn) {
    confirmAction('Chuyển đổi Web Engine', `Chuyển Web Server cổng 80/443 sang ${target}? Hệ thống sẽ dừng engine cũ, kiểm tra cấu hình và tự rollback nếu lỗi.`, () => withBusy(btn, async () => {
      try {
        await api('POST', '/api/webserver/switch', { targetEngine: target });
        toast(`Đã chuyển Web Engine sang ${target}.`);
        await renderWebserver(container);
      } catch (err) {
        toast(err.message, 'err');
      }
    }), false);
  }

  const swNginx = document.getElementById('ws-switch-nginx');
  const swFx = document.getElementById('ws-switch-ferrousx');
  swNginx?.addEventListener('click', () => doSwitch('nginx', swNginx));
  swFx?.addEventListener('click', () => doSwitch('ferrousx', swFx));

  const valBtn = document.getElementById('ws-validate-nginx');
  valBtn?.addEventListener('click', () => withBusy(valBtn, async () => {
    try {
      const res = await api('POST', '/api/nginx/validate', {});
      toast(res.message || 'Cấu hình Nginx hợp lệ.');
    } catch (err) {
      toast(err.message, 'err');
    }
  }));

  const syncBtn = document.getElementById('ws-sync-fx');
  syncBtn?.addEventListener('click', () => withBusy(syncBtn, async () => {
    try {
      await api('POST', '/api/ferrousx/sync-status', {});
      toast('Đã đồng bộ cấu hình FerrousX.');
      await renderWebserver(container);
    } catch (err) {
      toast(err.message, 'err');
    }
  }));

  const ipcReloadBtn = document.getElementById('ws-fx-reload');
  ipcReloadBtn?.addEventListener('click', () => withBusy(ipcReloadBtn, async () => {
    try {
      await api('POST', '/api/ferrousx/ipc/command', { command: 'reload' });
      toast('Đã gửi lệnh reload qua FerrousX UDS IPC.');
      await renderWebserver(container);
    } catch (err) {
      toast(err.message, 'err');
    }
  }));
}

// ============================================================================
// 6. PHP-FPM & SOFTWARE STORE (SETTINGS + SWITCH-SITE + CONFIG EDITOR)
// ============================================================================
export async function renderPhpSoftware(container) {
  const [phpList, pkgs, apps] = await Promise.all([
    api('GET', '/api/php').catch(() => []),
    api('GET', '/api/software').catch(() => []),
    api('GET', '/api/apps').catch(() => []),
  ]);

  let tab = 'software';

  function draw() {
    container.innerHTML = `
      <div class="page-head">
        <div>
          <h1 class="page-title">PHP-FPM & Kho Phần mềm Hệ thống</h1>
          <p class="page-desc">Quản lý gói dịch vụ hệ thống, cấu hình php.ini có kiểm tra php-fpm -t + rollback và chuyển đổi phiên bản PHP từng site.</p>
        </div>
        <div class="row">
          <div class="tabs">
            <button type="button" class="tab-btn ${tab === 'software' ? 'active' : ''}" id="sw-tab-pkgs">Phần mềm (${pkgs.length})</button>
            <button type="button" class="tab-btn ${tab === 'php' ? 'active' : ''}" id="sw-tab-php">PHP-FPM (${phpList.length})</button>
          </div>
          ${tab === 'php' ? `<button type="button" class="btn btn-primary btn-sm" id="php-switch-site-btn">Đổi PHP cho Website</button>` : ''}
        </div>
      </div>

      ${tab === 'php' ? capBannerHtml(['php_fpm']) : ''}

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
                            <button type="button" class="btn btn-sm" data-pkg-cfg="${esc(p.id)}">Cấu hình</button>
                            ${p.status === 'not_installed' || p.status === 'missing'
                              ? `<button type="button" class="btn btn-sm btn-primary" data-pkg-act="install" data-id="${esc(p.id)}" data-name="${esc(p.name)}">Cài đặt</button>`
                              : (['pkg-bun', 'pkg-tailwindcss', 'pkg-deno'].includes(p.id)
                                ? `<button type="button" class="btn btn-sm" data-pkg-act="install" data-id="${esc(p.id)}" data-name="${esc(p.name)}">Cập nhật</button>`
                                : `<button type="button" class="btn btn-sm" data-pkg-act="restart" data-id="${esc(p.id)}" data-name="${esc(p.name)}">Restart</button>
                                   <button type="button" class="btn btn-sm btn-danger" data-pkg-act="stop" data-id="${esc(p.id)}" data-name="${esc(p.name)}">Stop</button>`)}
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
                      <button type="button" class="btn btn-sm" data-php-cfg="${esc(p.version)}">Chỉnh php.ini</button>
                      <button type="button" class="btn btn-sm" data-php-act="restart" data-ver="${esc(p.version)}" ${p.installed ? '' : 'disabled'}>Restart</button>
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
                      <button type="button" class="btn btn-sm" data-php-ext="${esc(ext.name)}" data-ver="${esc(p.version)}" ${p.installed ? '' : 'disabled'}>
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

    document.getElementById('php-switch-site-btn')?.addEventListener('click', () => {
      openModal('Đổi phiên bản PHP-FPM cho Website', `
        <form id="php-sw-form">
          <div class="field">
            <label>Website</label>
            <select class="select" name="appId" required>
              ${(Array.isArray(apps) ? apps : []).map((a) => `<option value="${esc(a.id)}">${esc(a.domain)} (${esc(a.framework)})</option>`).join('')}
            </select>
          </div>
          <div class="field">
            <label>Phiên bản PHP-FPM</label>
            <select class="select" name="phpVersion">
              ${phpList.map((p) => `<option value="${esc(p.version)}">PHP ${esc(p.version)} (${p.installed ? p.status : 'chưa cài'})</option>`).join('')}
            </select>
          </div>
          <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="php-sw-sub">Áp dụng & Reload Vhost</button></div>
        </form>
      `, {
        onMount(body) {
          body.querySelector('#php-sw-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            await withBusy(body.querySelector('#php-sw-sub'), async () => {
              const fd = new FormData(e.target);
              try {
                await api('POST', '/api/php/switch-site', {
                  appId: fd.get('appId'),
                  phpVersion: fd.get('phpVersion'),
                });
                closeModal();
                toast('Đã chuyển phiên bản PHP-FPM cho website.');
                await renderPhpSoftware(container);
              } catch (err) { toast(err.message, 'err'); }
            });
          });
        },
      });
    });

    container.querySelectorAll('[data-pkg-cfg]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const pkg = pkgs.find((x) => x.id === btn.dataset.pkgCfg);
        if (!pkg) return;
        openModal(`Cấu hình: ${pkg.name} (${pkg.configPath || ''})`, `
          <form id="pkg-cfg-form">
            <div class="field"><label>Nội dung file cấu hình (tự rollback nếu lỗi cú pháp)</label><textarea class="textarea" name="configContent" style="min-height:280px">${esc(pkg.configContent || '')}</textarea></div>
            <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="pkg-cfg-sub">Lưu & Kiểm tra</button></div>
          </form>
        `, {
          large: true,
          onMount(body) {
            body.querySelector('#pkg-cfg-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              await withBusy(body.querySelector('#pkg-cfg-sub'), async () => {
                const configContent = body.querySelector('textarea[name="configContent"]').value;
                try {
                  await api('PUT', `/api/software/${encodeURIComponent(pkg.id)}/config`, { configContent });
                  closeModal();
                  toast(`Đã lưu cấu hình ${pkg.name}.`);
                  await renderPhpSoftware(container);
                } catch (err) { toast(err.message, 'err'); }
              });
            });
          },
        });
      });
    });

    container.querySelectorAll('[data-php-cfg]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const inst = phpList.find((x) => x.version === btn.dataset.phpCfg);
        if (!inst) return;
        openModal(`Cấu hình PHP ${inst.version} FPM (php-fpm -t + rollback)`, `
          <form id="php-cfg-form">
            <div class="form-grid">
              <div class="field"><label>memory_limit</label><input class="input mono" name="memoryLimit" value="${esc(inst.memoryLimit || '256M')}" /></div>
              <div class="field"><label>upload_max_filesize</label><input class="input mono" name="uploadMaxFilesize" value="${esc(inst.uploadMaxFilesize || '64M')}" /></div>
              <div class="field"><label>post_max_size</label><input class="input mono" name="postMaxSize" value="${esc(inst.postMaxSize || '64M')}" /></div>
              <div class="field"><label>max_execution_time (s)</label><input class="input mono" name="maxExecutionTime" type="number" value="${esc(inst.maxExecutionTime || 60)}" /></div>
              <div class="field"><label>pm.max_children</label><input class="input mono" name="maxChildren" type="number" value="${esc(inst.maxChildren || 32)}" /></div>
              <div class="field"><label>opcache.jit_buffer_size</label><input class="input mono" name="opcacheJitBuffer" value="${esc(inst.opcacheJitBuffer || '128M')}" /></div>
            </div>
            <div class="row" style="margin-bottom:12px">
              <label class="checkbox-row"><input type="checkbox" name="opcacheEnabled" ${inst.opcacheEnabled ? 'checked' : ''} /><span>Bật Zend OPcache</span></label>
              <label class="checkbox-row"><input type="checkbox" name="displayErrors" ${inst.displayErrors ? 'checked' : ''} /><span>display_errors</span></label>
            </div>
            <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="php-cfg-sub">Lưu & Kiểm tra php-fpm -t</button></div>
          </form>
        `, {
          onMount(body) {
            body.querySelector('#php-cfg-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              await withBusy(body.querySelector('#php-cfg-sub'), async () => {
                const fd = new FormData(e.target);
                try {
                  await api('PUT', `/api/php/${encodeURIComponent(inst.version)}/settings`, {
                    memoryLimit: fd.get('memoryLimit'),
                    uploadMaxFilesize: fd.get('uploadMaxFilesize'),
                    postMaxSize: fd.get('postMaxSize'),
                    maxExecutionTime: Number(fd.get('maxExecutionTime') || 60),
                    maxChildren: Number(fd.get('maxChildren') || 32),
                    opcacheJitBuffer: fd.get('opcacheJitBuffer'),
                    opcacheEnabled: fd.get('opcacheEnabled') === 'on',
                    displayErrors: fd.get('displayErrors') === 'on',
                  });
                  closeModal();
                  toast(`Đã lưu và xác nhận cấu hình PHP ${inst.version}.`);
                  await renderPhpSoftware(container);
                } catch (err) { toast(err.message, 'err'); }
              });
            });
          },
        });
      });
    });

    async function waitForPanelReconnect(maxWaitMs = 10000) {
      const deadline = Date.now() + maxWaitMs;
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 700));
        try {
          const resp = await fetch('/api/health', { cache: 'no-store', credentials: 'same-origin' });
          if (resp.ok) return true;
        } catch (_) {
          // still restarting
        }
      }
      return false;
    }

    container.querySelectorAll('[data-pkg-act]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const pkgId = btn.dataset.id;
        const pkgName = btn.dataset.name || pkgId;
        const act = btn.dataset.pkgAct;
        const isWebEngine = pkgId === 'pkg-ferrousx' || pkgId === 'pkg-nginx';

        const runAct = () => withBusy(btn, async () => {
          try {
            await api('POST', `/api/software/${encodeURIComponent(pkgId)}/action`, { action: act });
            toast(`Đã thực thi '${act}' cho ${pkgName}.`);
            await renderPhpSoftware(container);
          } catch (err) {
            const transientProxyDrop =
              isWebEngine &&
              act === 'restart' &&
              (!err.status || err.status === 502 || err.status === 503 || err.status === 504);
            if (transientProxyDrop) {
              toast(`Đang khởi động lại ${pkgName}, chờ kết nối lại…`);
              const back = await waitForPanelReconnect(10000);
              if (back) {
                toast(`${pkgName} đã khởi động lại và hoạt động bình thường.`);
                await renderPhpSoftware(container);
                return;
              }
            }
            toast(err.message, 'err');
          }
        });

        if (isWebEngine && act === 'stop') {
          confirmAction(
            `Dừng ${pkgName}`,
            `Dừng ${pkgName} sẽ tạm ngắt cổng 80/443 (có thể làm mất kết nối nếu bạn truy cập Panel qua tên miền proxy). Bạn chắc chắn muốn dừng?`,
            runAct,
          );
        } else {
          runAct();
        }
      });
    });

    container.querySelectorAll('[data-php-act]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          await api('POST', `/api/php/${encodeURIComponent(btn.dataset.ver)}/action`, { action: btn.dataset.phpAct });
          toast(`Đã thực thi '${btn.dataset.phpAct}' cho PHP ${btn.dataset.ver}.`);
          await renderPhpSoftware(container);
        } catch (err) { toast(err.message, 'err'); }
      }));
    });

    container.querySelectorAll('[data-php-ext]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          await api('POST', `/api/php/${encodeURIComponent(btn.dataset.ver)}/extensions/toggle`, { extName: btn.dataset.phpExt });
          toast(`Đã cập nhật extension ${btn.dataset.phpExt}.`);
          await renderPhpSoftware(container);
        } catch (err) { toast(err.message, 'err'); }
      }));
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
        <p class="page-desc">Lập lịch crontab hệ thống, sao lưu nén tar.zst bất đồng bộ kèm SHA-256 và quản trị bản ghi DNS / SFTP.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="cp-add-cron" ${capDisabledAttr('cron_backup')}>Thêm Cron Job</button>
        <button type="button" class="btn btn-primary" id="cp-create-backup" ${capDisabledAttr('cron_backup')}>Tạo Backup nền (Async)</button>
      </div>
    </div>

    ${capBannerHtml(['cron_backup'])}

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Crontab (${crons.length})</h2></div>
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
                          <button type="button" class="btn btn-sm btn-danger" data-cron-del="${esc(c.id)}" data-title="${esc(c.title)}">Xóa</button>
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
        <div class="surface-head"><h2 class="surface-title">Bản sao lưu Zstd (${backups.length})</h2></div>
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
                          <button type="button" class="btn btn-sm btn-danger" data-bk-del="${esc(b.id)}" data-name="${esc(b.filename || b.id)}">Xóa</button>
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
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-dns-del="${esc(r.id)}" data-name="${esc(r.name)}">Xóa</button></td>
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
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-sftp-del="${esc(u.id)}" data-name="${esc(u.username)}">Xóa</button></td>
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
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="cron-sub">Lưu Cron</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#cron-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#cron-sub'), async () => {
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
              toast('Đã thêm Cron Job vào hệ thống.');
              await renderCpanel(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('cp-create-backup')?.addEventListener('click', () => {
    openModal('Tạo bản sao lưu nền (Zstd + SHA-256)', `
      <form id="bk-form">
        <div class="field">
          <label>Website mục tiêu</label>
          ${apps.length
            ? `<select class="select" name="targetApp">${apps.map((a) => `<option value="${esc(a.domain)}">${esc(a.domain)}</option>`).join('')}</select>`
            : '<input class="input" name="targetApp" placeholder="example.com" required />'}
        </div>
        <div class="field"><label>Kèm Database (tùy chọn)</label><input class="input mono" name="databaseIncluded" placeholder="Để trống nếu chỉ sao lưu mã nguồn" /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="bk-sub">Chạy sao lưu nền</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#bk-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#bk-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              const job = await api('POST', '/api/jobs/cpanel/backups', {
                targetApp: fd.get('targetApp'),
                databaseIncluded: fd.get('databaseIncluded') || '',
              });
              closeModal();
              toast('Đã xếp hàng tác vụ sao lưu.');
              openJobDrawer(job.id);
            } catch (err) { toast(err.message, 'err'); }
          });
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
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="dns-sub">Lưu bản ghi</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#dns-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#dns-sub'), async () => {
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
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('cp-add-sftp')?.addEventListener('click', () => {
    openModal('Thêm tài khoản SFTP', `
      <form id="sftp-form">
        <div class="field"><label>Tên tài khoản</label><input class="input mono" name="username" placeholder="deploy_user" required /></div>
        <div class="field"><label>Thư mục gốc</label><input class="input mono" name="rootPath" placeholder="/www/wwwroot/example.com" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="sftp-sub">Tạo SFTP</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#sftp-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#sftp-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/cpanel/sftp', { username: fd.get('username'), rootPath: fd.get('rootPath') });
              closeModal();
              toast('Đã thêm tài khoản SFTP.');
              await renderCpanel(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  container.querySelectorAll('[data-cron-run]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('POST', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronRun)}/run`, {}); toast('Đã chạy thử Cron Job.'); }
    catch (err) { toast(err.message, 'err'); }
  })));
  container.querySelectorAll('[data-cron-toggle]').forEach((b) => b.addEventListener('click', () => withBusy(b, async () => {
    try { await api('PUT', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronToggle)}/toggle`, {}); await renderCpanel(container); }
    catch (err) { toast(err.message, 'err'); }
  })));
  container.querySelectorAll('[data-cron-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa Cron Job', `Xác nhận xóa lịch Cron "${b.dataset.title}"?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/cpanel/cron/${encodeURIComponent(b.dataset.cronDel)}`); await renderCpanel(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
  container.querySelectorAll('[data-bk-restore]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Khôi phục bản sao lưu', 'Xác nhận kiểm tra SHA-256 và khôi phục dữ liệu từ bản sao lưu này?', () => withBusy(b, async () => {
      try { await api('POST', `/api/cpanel/backups/${encodeURIComponent(b.dataset.bkRestore)}/restore`, {}); toast('Đã khôi phục bản sao lưu.'); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
  container.querySelectorAll('[data-bk-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa bản sao lưu', `Xác nhận xóa vĩnh viễn tệp sao lưu ${b.dataset.name}?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/cpanel/backups/${encodeURIComponent(b.dataset.bkDel)}`); await renderCpanel(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
  container.querySelectorAll('[data-dns-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa bản ghi DNS', `Xác nhận xóa bản ghi DNS ${b.dataset.name}?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/cpanel/dns/${encodeURIComponent(b.dataset.dnsDel)}`); await renderCpanel(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
  container.querySelectorAll('[data-sftp-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa tài khoản SFTP', `Xác nhận thu hồi và xóa tài khoản SFTP ${b.dataset.name}?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/cpanel/sftp/${encodeURIComponent(b.dataset.sftpDel)}`); await renderCpanel(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
}

// ============================================================================
// 8. MAIL SERVER (REAL DKIM + REAL DNS STATUS + FORWARDERS + SEND TEST)
// ============================================================================
export async function renderMail(container) {
  const data = await api('GET', '/api/mail');
  const domains = data.domains || [];
  const mailboxes = data.mailboxes || [];
  const forwarders = data.forwarders || [];
  const mailDis = capDisabledAttr('mail_server');

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Máy chủ Mail (Postfix / Dovecot / Rspamd)</h1>
        <p class="page-desc">Quản lý tên miền thư điện tử với khóa DKIM RSA-2048 thật, kiểm chứng DNS TXT (SPF/DKIM/DMARC), hộp thư và forwarders.</p>
      </div>
      <div class="row">
        <button type="button" class="btn" id="mail-add-dom" ${mailDis}>Thêm tên miền</button>
        <button type="button" class="btn" id="mail-add-fwd" ${mailDis}>Thêm Forwarder</button>
        <button type="button" class="btn" id="mail-send-test" ${mailDis}>Gửi Mail thử</button>
        <button type="button" class="btn btn-primary" id="mail-add-box" ${mailDis}>Tạo hộp thư</button>
      </div>
    </div>

    ${capBannerHtml(['mail_server'])}

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">Tên miền Mail & Kiểm chứng DNS (${domains.length})</h2>
        <div class="meta-line">
          <span>Postfix: <strong class="${statusClass(data.postfixStatus || 'unknown')}">${esc(data.postfixStatus || '—')}</strong></span>
          <span>·</span>
          <span>Dovecot: <strong class="${statusClass(data.dovecotStatus || 'unknown')}">${esc(data.dovecotStatus || '—')}</strong></span>
          <span>·</span>
          <span>Rspamd: <strong class="${statusClass(data.rspamdStatus || 'unknown')}">${esc(data.rspamdStatus || '—')}</strong></span>
        </div>
      </div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Tên miền</th><th>SPF / DKIM / DMARC (DNS TXT thật)</th><th>DKIM Public Record</th></tr></thead>
          <tbody>
            ${
              domains.length
                ? domains.map((d) => `
                  <tr>
                    <td class="mono"><strong>${esc(d.domain)}</strong></td>
                    <td class="meta-line">
                      <span>SPF: <strong class="${statusClass(d.spfStatus)}">${esc(d.spfStatus || 'unverified')}</strong></span>
                      <span>·</span>
                      <span>DKIM: <strong class="${statusClass(d.dkimStatus)}">${esc(d.dkimStatus || 'unverified')}</strong></span>
                      <span>·</span>
                      <span>DMARC: <strong class="${statusClass(d.dmarcStatus)}">${esc(d.dmarcStatus || 'unverified')}</strong></span>
                    </td>
                    <td class="mono" style="max-width:360px;word-break:break-all;font-size:11px">${esc(d.dkimSelector || 'ferrous._domainkey')}: ${esc((d.dkimPublicKey || '').slice(0, 72))}…</td>
                  </tr>`).join('')
                : '<tr><td colspan="3" class="empty-state">Chưa cấu hình tên miền Mail.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Hộp thư (${mailboxes.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Địa chỉ Email</th><th class="right">Hạn mức</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                mailboxes.length
                  ? mailboxes.map((m) => `
                    <tr>
                      <td class="mono"><strong>${esc(m.email)}</strong></td>
                      <td class="right mono">${esc(m.quotaMb || 1024)} MB</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-mb-del="${esc(m.id)}" data-email="${esc(m.email)}" ${mailDis}>Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có hộp thư nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">Chuyển tiếp Mail / Forwarders (${forwarders.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Nguồn</th><th>Đích</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                forwarders.length
                  ? forwarders.map((f) => `
                    <tr>
                      <td class="mono">${esc(f.sourceEmail)}</td>
                      <td class="mono">${esc(f.destinationEmail)}</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-fwd-del="${esc(f.id)}" data-src="${esc(f.sourceEmail)}" ${mailDis}>Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có forwarder nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>
  `;

  document.getElementById('mail-add-dom')?.addEventListener('click', () => {
    openModal('Thêm tên miền Mail (sinh khóa RSA-2048 DKIM)', `
      <form id="md-form">
        <div class="field"><label>Tên miền</label><input class="input mono" name="domain" placeholder="example.com" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="md-sub">Thêm tên miền</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#md-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#md-sub'), async () => {
            const domain = new FormData(e.target).get('domain');
            try {
              await api('POST', '/api/mail/domains', { domain });
              closeModal();
              toast('Đã thêm tên miền mail và sinh khóa DKIM RSA-2048.');
              await renderMail(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('mail-add-box')?.addEventListener('click', () => {
    openModal('Tạo hộp thư mới', `
      <form id="mb-form">
        <div class="form-grid">
          <div class="field"><label>Địa chỉ Email</label><input class="input mono" name="email" placeholder="info@example.com" required /></div>
          <div class="field"><label>Tên hiển thị</label><input class="input" name="displayName" placeholder="Support Team" /></div>
          <div class="field"><label>Hạn mức (MB)</label><input class="input mono" name="quotaMb" type="number" value="2048" /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="mb-sub">Tạo hộp thư</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#mb-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#mb-sub'), async () => {
            const fd = new FormData(e.target);
            const email = String(fd.get('email') || '').trim();
            const domain = email.includes('@') ? email.split('@')[1] : (domains[0]?.domain || 'example.com');
            try {
              await api('POST', '/api/mail/mailboxes', {
                email,
                domain,
                displayName: fd.get('displayName') || '',
                quotaMb: Number(fd.get('quotaMb') || 2048),
              });
              closeModal();
              toast('Đã tạo hộp thư mới.');
              await renderMail(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('mail-add-fwd')?.addEventListener('click', () => {
    openModal('Thêm quy tắc chuyển tiếp Mail (Forwarder)', `
      <form id="fwd-form">
        <div class="field"><label>Email nguồn</label><input class="input mono" name="sourceEmail" placeholder="support@example.com" required /></div>
        <div class="field"><label>Email đích</label><input class="input mono" name="destinationEmail" placeholder="admin@example.com" required /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="fwd-sub">Lưu Forwarder</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#fwd-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#fwd-sub'), async () => {
            const fd = new FormData(e.target);
            const src = String(fd.get('sourceEmail') || '').trim();
            const domain = src.includes('@') ? src.split('@')[1] : (domains[0]?.domain || 'example.com');
            try {
              await api('POST', '/api/mail/forwarders', {
                sourceEmail: src,
                destinationEmail: fd.get('destinationEmail'),
                domain,
              });
              closeModal();
              toast('Đã thêm forwarder.');
              await renderMail(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('mail-send-test')?.addEventListener('click', () => {
    openModal('Gửi Email kiểm thử qua Postfix/Sendmail', `
      <form id="msend-form">
        <div class="form-grid">
          <div class="field"><label>Từ (From)</label><input class="input mono" name="from" placeholder="admin@example.com" required /></div>
          <div class="field"><label>Đến (To)</label><input class="input mono" name="to" placeholder="you@gmail.com" required /></div>
        </div>
        <div class="field"><label>Tiêu đề</label><input class="input" name="subject" value="FerrousPanel SMTP Test" required /></div>
        <div class="field"><label>Nội dung</label><textarea class="textarea" name="body" style="min-height:90px">Kiểm thử gửi thư từ FerrousPanel Postfix MTA.</textarea></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="msend-sub">Gửi thư</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#msend-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#msend-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/mail/send', {
                from: fd.get('from'),
                to: fd.get('to'),
                subject: fd.get('subject'),
                body: fd.get('body'),
              });
              closeModal();
              toast('Đã gửi email kiểm thử thành công.');
              await renderMail(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  container.querySelectorAll('[data-mb-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa hộp thư', `Xác nhận xóa hộp thư ${b.dataset.email}?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/mail/mailboxes/${encodeURIComponent(b.dataset.mbDel)}`); await renderMail(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));

  container.querySelectorAll('[data-fwd-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa Forwarder', `Xác nhận xóa chuyển tiếp từ ${b.dataset.src}?`, () => withBusy(b, async () => {
      try { await api('DELETE', `/api/mail/forwarders/${encodeURIComponent(b.dataset.fwdDel)}`); await renderMail(container); }
      catch (err) { toast(err.message, 'err'); }
    }));
  }));
}

// ============================================================================
// 9. EDGE CLOUD & POLYGLOT FFI (EXPERIMENTAL / OPTIONAL)
// ============================================================================
export async function renderEdgePolyglot(container) {
  const [edge, toolchains] = await Promise.all([
    api('GET', '/api/edge-cloud').catch(() => ({})),
    api('GET', '/api/polyglot/toolchains').catch(() => []),
  ]);

  const workers = edge.wasiWorkers || [];
  const containers = edge.ociContainers || [];
  const peers = edge.wireguardPeers || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Edge Cloud & Polyglot FFI (Tùy chọn / Thử nghiệm)</h1>
        <p class="page-desc">Các tính năng mở rộng yêu cầu binary tương ứng trên VPS (wasmtime, podman/docker, wireguard-tools, rustc). Nút sẽ bị khóa nếu thiếu công cụ.</p>
      </div>
      <div class="row">
        <button type="button" class="btn btn-sm" id="edge-add-oci" ${capDisabledAttr('edge_cloud_oci')}>Chạy OCI Container</button>
        <button type="button" class="btn btn-sm" id="edge-add-wg" ${capDisabledAttr('edge_cloud_wireguard')}>Thêm WireGuard Peer</button>
        <button type="button" class="btn btn-sm btn-primary" id="edge-compile-ffi" ${capDisabledAttr('polyglot_ffi')}>Biên dịch Polyglot FFI (.so)</button>
      </div>
    </div>

    ${capBannerHtml(['edge_cloud_wasi', 'edge_cloud_oci', 'edge_cloud_wireguard', 'polyglot_ffi'])}

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">OCI Containers (${containers.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Tên & Image</th><th>Trạng thái</th><th class="right">Thao tác</th></tr></thead>
            <tbody>
              ${
                containers.length
                  ? containers.map((c) => `
                    <tr>
                      <td><strong>${esc(c.name)}</strong><div class="meta-line mono">${esc(c.image)}</div></td>
                      <td><span class="${statusClass(c.status)}">${esc(c.status)}</span></td>
                      <td class="right">
                        <div class="row" style="justify-content:flex-end">
                          <button type="button" class="btn btn-sm" data-oci-act="restart" data-id="${esc(c.id)}" ${capDisabledAttr('edge_cloud_oci')}>Restart</button>
                          <button type="button" class="btn btn-sm btn-danger" data-oci-act="delete" data-id="${esc(c.id)}" ${capDisabledAttr('edge_cloud_oci')}>Xóa</button>
                        </div>
                      </td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có container OCI nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>

      <section class="surface">
        <div class="surface-head"><h2 class="surface-title">WireGuard Peers (${peers.length}) & WASI (${workers.length})</h2></div>
        <div class="table-wrap">
          <table class="data-table">
            <thead><tr><th>Peer</th><th>Allowed IPs</th><th class="right">Xóa</th></tr></thead>
            <tbody>
              ${
                peers.length
                  ? peers.map((p) => `
                    <tr>
                      <td><strong>${esc(p.name)}</strong><div class="meta-line mono">${esc(p.publicKey?.slice(0, 24) || '')}…</div></td>
                      <td class="mono">${esc(p.allowedIps)}</td>
                      <td class="right"><button type="button" class="btn btn-sm btn-danger" data-wg-del="${esc(p.id)}" ${capDisabledAttr('edge_cloud_wireguard')}>Xóa</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có WireGuard peer nào.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <section class="surface">
      <div class="surface-head"><h2 class="surface-title">Polyglot FFI Toolchains (${toolchains.length})</h2></div>
      <div class="table-wrap">
        <table class="data-table">
          <thead><tr><th>Ngôn ngữ đích</th><th>Giao thức Bridge</th><th>Trạng thái</th></tr></thead>
          <tbody>
            ${
              toolchains.length
                ? toolchains.map((t) => `
                  <tr>
                    <td><strong>${esc(t.language)}</strong></td>
                    <td class="mono">${esc(t.bridgeProtocol)}</td>
                    <td><span class="${statusClass(t.status)}">${esc(t.status)}</span></td>
                  </tr>`).join('')
                : '<tr><td colspan="3" class="empty-state">Không có toolchain nào.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('edge-add-oci')?.addEventListener('click', () => {
    openModal('Triển khai OCI Container (Podman/Docker)', `
      <form id="oci-form">
        <div class="form-grid">
          <div class="field"><label>Tên container</label><input class="input mono" name="name" placeholder="cache-01" required /></div>
          <div class="field"><label>Image</label><input class="input mono" name="image" placeholder="docker.io/library/redis:7-alpine" required /></div>
          <div class="field"><label>Host Port</label><input class="input mono" name="hostPort" type="number" value="6379" /></div>
          <div class="field"><label>Container Port</label><input class="input mono" name="containerPort" type="number" value="6379" /></div>
        </div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="oci-sub">Triển khai</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#oci-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#oci-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/edge-cloud/containers', {
                name: fd.get('name'),
                image: fd.get('image'),
                runtimeEngine: 'podman',
                hostPort: Number(fd.get('hostPort') || 6379),
                containerPort: Number(fd.get('containerPort') || 6379),
                memoryMaxMb: 256,
                cpuQuotaPercent: 100,
              });
              closeModal();
              toast('Đã khởi chạy OCI container.');
              await renderEdgePolyglot(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('edge-add-wg')?.addEventListener('click', () => {
    openModal('Thêm WireGuard Peer', `
      <form id="wg-form">
        <div class="field"><label>Tên node</label><input class="input mono" name="name" placeholder="edge-node-01" required /></div>
        <div class="field"><label>Allowed IPs</label><input class="input mono" name="allowedIps" value="10.66.0.10/32" required /></div>
        <div class="field"><label>Endpoint (tùy chọn)</label><input class="input mono" name="endpoint" placeholder="203.0.113.10:51820" /></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="wg-sub">Tạo Peer</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#wg-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#wg-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/edge-cloud/wireguard', {
                name: fd.get('name'),
                region: 'edge',
                endpoint: fd.get('endpoint') || '',
                allowedIps: fd.get('allowedIps'),
              });
              closeModal();
              toast('Đã tạo WireGuard peer với khóa Curve25519 thật.');
              await renderEdgePolyglot(container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  document.getElementById('edge-compile-ffi')?.addEventListener('click', () => {
    openModal('Biên dịch Rust C-ABI FFI Bridge (.so)', `
      <form id="ffi-form">
        <div class="field"><label>Tên module Rust</label><input class="input mono" name="moduleName" value="ferrous_bridge" required /></div>
        <div class="field"><label>Ngôn ngữ đích</label><select class="select" name="targetLanguage"><option>TypeScript / Bun</option><option>Python</option><option>PHP</option><option>Go</option></select></div>
        <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="ffi-sub">Biên dịch (.so)</button></div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#ffi-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#ffi-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              const res = await api('POST', '/api/polyglot/compile', {
                moduleName: fd.get('moduleName'),
                targetLanguage: fd.get('targetLanguage'),
                optimizationLevel: 'release',
                enableZeroCopyIpc: true,
                customFunctions: ['verify_token'],
              });
              closeModal();
              openModal('Kết quả biên dịch FFI', `<pre class="terminal-box">${esc(res.compilerOutput || '')}</pre>`, { large: true });
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      },
    });
  });

  container.querySelectorAll('[data-oci-act]').forEach((b) => b.addEventListener('click', () => {
    const act = b.dataset.ociAct;
    const run = () => withBusy(b, async () => {
      try {
        await api('POST', `/api/edge-cloud/containers/${encodeURIComponent(b.dataset.id)}/action`, { action: act });
        toast(`Đã thực thi '${act}' trên container.`);
        await renderEdgePolyglot(container);
      } catch (err) { toast(err.message, 'err'); }
    });
    if (act === 'delete') confirmAction('Xóa Container', 'Xác nhận xóa OCI container này?', run);
    else run();
  }));

  container.querySelectorAll('[data-wg-del]').forEach((b) => b.addEventListener('click', () => {
    confirmAction('Xóa WireGuard Peer', 'Xác nhận xóa peer này khỏi giao diện wg0?', () => withBusy(b, async () => {
      try {
        await api('DELETE', `/api/edge-cloud/wireguard/${encodeURIComponent(b.dataset.wgDel)}`);
        await renderEdgePolyglot(container);
      } catch (err) { toast(err.message, 'err'); }
    }));
  }));
}
