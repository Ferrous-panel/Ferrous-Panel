// FerrousPanel Pure-Rust UI — Core Views (Overview, Websites, Files, SQL Databases)
import {
  api, esc, fmtBytes, fmtTime, fmtUptime, statusClass,
  toast, openModal, closeModal, confirmAction, openJobDrawer, navigate, state,
  capBannerHtml, capDisabledAttr, withBusy,
} from './app.js';

function sparklineSvg(values, color = 'var(--accent)') {
  if (!values || values.length < 2) {
    return '<div class="empty-state" style="padding:18px 0">Đang thu thập mẫu telemetry…</div>';
  }
  const w = 420;
  const h = 76;
  const max = Math.max(100, ...values);
  const step = w / (values.length - 1);
  const pts = values.map((v, i) => `${(i * step).toFixed(1)},${(h - (Math.min(max, Math.max(0, v)) / max) * (h - 10) - 5).toFixed(1)}`).join(' ');
  return `
    <svg class="spark-svg" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true">
      <polyline fill="none" stroke="${color}" stroke-width="2" stroke-linejoin="round" points="${pts}" />
    </svg>
  `;
}

// ============================================================================
// 1. OVERVIEW / TELEMETRY + CAPABILITY MATRIX
// ============================================================================
export async function renderOverview(container) {
  const [tel, ws, apps, dbs, jobs] = await Promise.all([
    api('GET', '/api/telemetry'),
    api('GET', '/api/webserver').catch(() => null),
    api('GET', '/api/apps').catch(() => []),
    api('GET', '/api/databases').catch(() => []),
    api('GET', '/api/jobs?limit=8').catch(() => []),
  ]);

  const cpuPct = Number(tel.cpuUsagePercent || 0);
  const memPct = Number(tel.memoryPercent || 0);
  const diskPct = tel.diskTotalGb > 0 ? (tel.diskUsedGb / tel.diskTotalGb) * 100 : 0;
  state.telemetryHistory.push({ cpu: cpuPct, mem: memPct });
  if (state.telemetryHistory.length > 32) state.telemetryHistory.shift();

  const activeEngine = ws?.frontendEngine || ws?.frontend_engine || 'nginx';
  const loadAvg = Array.isArray(tel.loadAverage) ? tel.loadAverage.map((v) => Number(v).toFixed(2)).join(' / ') : '0.00 / 0.00 / 0.00';
  const caps = state.capabilities?.capabilities || [];

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Tổng quan hệ thống</h1>
        <div class="meta-line">
          <span class="mono">${esc(tel.hostname || 'vps')}</span>
          <span>·</span>
          <span class="mono">${esc(tel.publicIp || '127.0.0.1')}</span>
          <span>·</span>
          <span>${esc(tel.osVersion || 'Ubuntu LTS')}</span>
          <span>·</span>
          <span>Kernel <span class="mono">${esc(tel.kernelVersion || '—')}</span></span>
          <span>·</span>
          <span>Uptime <span class="mono">${esc(fmtUptime(tel.uptimeSeconds))}</span></span>
        </div>
      </div>
      <div class="row">
        <button type="button" class="btn btn-primary" id="ov-new-app">Triển khai Website</button>
        <button type="button" class="btn" id="ov-open-db">Quản lý SQL</button>
      </div>
    </div>

    <div class="kpi-grid">
      <div class="kpi-card">
        <div class="kpi-label">CPU (${esc(tel.cpuCores || 1)} vCPU)</div>
        <div class="kpi-val">${cpuPct.toFixed(1)}%</div>
        <div class="bar-track"><div class="bar-fill ${cpuPct > 85 ? 'err' : cpuPct > 65 ? 'warn' : 'ok'}" style="width:${Math.min(100, cpuPct)}%"></div></div>
        <div class="kpi-sub mono">Load: ${esc(loadAvg)}</div>
      </div>
      <div class="kpi-card">
        <div class="kpi-label">Bộ nhớ RAM</div>
        <div class="kpi-val">${memPct.toFixed(1)}%</div>
        <div class="bar-track"><div class="bar-fill ${memPct > 85 ? 'err' : memPct > 70 ? 'warn' : 'ok'}" style="width:${Math.min(100, memPct)}%"></div></div>
        <div class="kpi-sub mono">${esc(tel.memoryUsedMb || 0)} / ${esc(tel.memoryTotalMb || 0)} MB</div>
      </div>
      <div class="kpi-card">
        <div class="kpi-label">Ổ đĩa lưu trữ</div>
        <div class="kpi-val">${diskPct.toFixed(1)}%</div>
        <div class="bar-track"><div class="bar-fill ${diskPct > 85 ? 'err' : diskPct > 70 ? 'warn' : 'ok'}" style="width:${Math.min(100, diskPct)}%"></div></div>
        <div class="kpi-sub mono">${Number(tel.diskUsedGb || 0).toFixed(1)} / ${Number(tel.diskTotalGb || 0).toFixed(1)} GB</div>
      </div>
      <div class="kpi-card">
        <div class="kpi-label">Website & Database</div>
        <div class="kpi-val">${Array.isArray(apps) ? apps.length : 0} / ${Array.isArray(dbs) ? dbs.length : 0}</div>
        <div class="kpi-sub">Web Engine: <strong class="mono">${esc(activeEngine)}</strong> · Mode: <span class="mono">${esc(state.capabilities?.mode || 'production')}</span></div>
      </div>
    </div>

    <div class="grid-2">
      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Biểu đồ tải CPU & RAM trực tiếp</h2>
          <span class="meta-line"><span>CPU ${cpuPct.toFixed(1)}%</span><span>·</span><span>RAM ${memPct.toFixed(1)}%</span></span>
        </div>
        <div>
          <div class="kpi-sub" style="margin-bottom:4px">CPU (%)</div>
          ${sparklineSvg(state.telemetryHistory.map((p) => p.cpu), 'var(--accent)')}
          <div class="kpi-sub" style="margin:8px 0 4px">RAM (%)</div>
          ${sparklineSvg(state.telemetryHistory.map((p) => p.mem), 'var(--ok)')}
        </div>
      </section>

      <section class="surface">
        <div class="surface-head">
          <h2 class="surface-title">Web Server & Tác vụ nền gần nhất</h2>
          <button type="button" class="btn btn-sm" id="ov-goto-ws">Cấu hình Engine</button>
        </div>
        <div class="meta-line" style="margin-bottom:12px">
          <span>Engine đang chạy: <strong class="mono">${esc(activeEngine)}</strong></span>
          <span>·</span>
          <span>Nginx: <span class="${statusClass(ws?.nginx?.active ? 'active' : 'stopped')}">${ws?.nginx?.active ? 'Active' : 'Inactive'}</span></span>
          <span>·</span>
          <span>FerrousX: <span class="${statusClass(ws?.ferrousx?.active ? 'active' : 'stopped')}">${ws?.ferrousx?.active ? 'Active' : 'Inactive'}</span></span>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr><th>Tác vụ</th><th>Trạng thái</th><th class="right">Chi tiết</th></tr>
            </thead>
            <tbody>
              ${
                Array.isArray(jobs) && jobs.length
                  ? jobs.slice(0, 5).map((j) => `
                    <tr>
                      <td>${esc(j.title)}</td>
                      <td><span class="${statusClass(j.status)}">${esc(j.status)}</span> · <span class="mono">${esc(j.progress)}%</span></td>
                      <td class="right"><button type="button" class="btn btn-sm" data-view-job="${esc(j.id)}">Xem log</button></td>
                    </tr>`).join('')
                  : '<tr><td colspan="3" class="empty-state">Chưa có tác vụ nền nào được chạy.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    </div>

    <section class="surface">
      <div class="surface-head">
        <h2 class="surface-title">Trạng thái khả dụng phân hệ (Feature Capabilities — kiểm tra thật trên VPS)</h2>
        <span class="meta-line"><span>Tổng: <strong class="mono">${caps.length}</strong> tính năng</span></span>
      </div>
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr><th>Phân hệ</th><th>Trạng thái</th><th>Lý do kiểm chứng</th><th>Lệnh gợi ý</th></tr>
          </thead>
          <tbody>
            ${
              caps.length
                ? caps.map((c) => `
                  <tr>
                    <td><strong>${esc(c.label || c.id)}</strong></td>
                    <td><span class="${statusClass(c.status)}">${esc(String(c.status || '').toUpperCase())}</span></td>
                    <td>${esc(c.reason || '—')}</td>
                    <td class="mono">${esc(c.fixHint || '—')}</td>
                  </tr>`).join('')
                : '<tr><td colspan="4" class="empty-state">Không tải được bảng trạng thái capabilities.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('ov-new-app')?.addEventListener('click', () => navigate('apps'));
  document.getElementById('ov-open-db')?.addEventListener('click', () => navigate('databases'));
  document.getElementById('ov-goto-ws')?.addEventListener('click', () => navigate('webserver'));
  container.querySelectorAll('[data-view-job]').forEach((btn) => {
    btn.addEventListener('click', () => openJobDrawer(btn.dataset.viewJob));
  });

  if (state.refreshTimer) clearInterval(state.refreshTimer);
  state.refreshTimer = setInterval(async () => {
    if (state.route !== 'overview') return;
    const modalOpen = Boolean(document.getElementById('modal-root')?.innerHTML);
    if (modalOpen) return;
    const y = window.scrollY;
    try {
      await renderOverview(container);
      window.scrollTo(0, y);
    } catch (_) {
      // silent background poll
    }
  }, 15000);
}

// ============================================================================
// 2. WEBSITES & APPS + 1-CLICK TEMPLATES + SITE SETTINGS
// ============================================================================
const RUNTIMES = [
  'Static HTML / SPA',
  'Vite + React 19 / Vue 3 SPA',
  'Next.js 15 (React SSR)',
  'Nuxt 3 (Vue 3 SSR)',
  'SvelteKit 2 (Svelte 5)',
  'Astro 5 (Hybrid / Static)',
  'Bun / ElysiaJS (TypeScript)',
  'Actix-Web (Rust)',
  'Axum (Rust)',
  'PHP 8.3-FPM',
  'Python / FastAPI',
  'Go / Fiber',
];

export async function renderApps(container) {
  const [apps, templates] = await Promise.all([
    api('GET', '/api/apps'),
    api('GET', '/api/oneclick/templates').catch(() => []),
  ]);

  let searchTerm = '';
  let activeTab = 'sites';

  function draw() {
    const filtered = (Array.isArray(apps) ? apps : []).filter((a) => {
      if (!searchTerm) return true;
      const q = searchTerm.toLowerCase();
      return String(a.domain || '').toLowerCase().includes(q)
        || String(a.name || '').toLowerCase().includes(q)
        || String(a.framework || '').toLowerCase().includes(q);
    });

    container.innerHTML = `
      <div class="page-head">
        <div>
          <h1 class="page-title">Website & Ứng dụng</h1>
          <p class="page-desc">Quản lý triển khai website Rust, PHP, Python, Go, Bun/JS/CSS và chứng chỉ SSL tự động trên cổng 80/443.</p>
        </div>
        <div class="row">
          <div class="tabs">
            <button type="button" class="tab-btn ${activeTab === 'sites' ? 'active' : ''}" id="tab-sites">Danh sách (${apps.length})</button>
            <button type="button" class="tab-btn ${activeTab === 'oneclick' ? 'active' : ''}" id="tab-oneclick">Triển khai 1-Click (${templates.length})</button>
          </div>
          <button type="button" class="btn btn-primary" id="btn-deploy-app">Thêm Website mới</button>
        </div>
      </div>

      ${capBannerHtml(['ssl_certbot'])}

      ${activeTab === 'sites' ? `
        <section class="surface">
          <div class="surface-head">
            <input class="input" id="apps-search" style="max-width:340px" placeholder="Tìm theo tên miền, ứng dụng hoặc framework…" value="${esc(searchTerm)}" />
            <span class="meta-line"><span>Tổng: <strong class="mono">${filtered.length}</strong> website</span></span>
          </div>
          <div class="table-wrap">
            <table class="data-table">
              <thead>
                <tr>
                  <th>Tên miền / Ứng dụng</th>
                  <th>Framework & Engine</th>
                  <th>Trạng thái</th>
                  <th>Cổng nội bộ</th>
                  <th class="right">Thao tác</th>
                </tr>
              </thead>
              <tbody>
                ${
                  filtered.length
                    ? filtered.map((a) => `
                      <tr>
                        <td>
                          <div><strong>${esc(a.domain)}</strong></div>
                          <div class="meta-line"><span>${esc(a.name)}</span><span>·</span><span class="mono">${esc(a.workingDir || '')}</span></div>
                        </td>
                        <td>
                          <div>${esc(a.framework)}</div>
                          <div class="meta-line">
                            <span>${esc(a.webServerEngine || 'nginx')}</span>
                            <span>·</span>
                            <span class="${a.sslEnabled ? 'status-ok' : 'muted'}">${a.sslEnabled ? (a.forceHttps ? 'HTTPS (301)' : 'HTTPS: Bật') : 'HTTP (chưa có SSL)'}</span>
                            ${a.rateLimitRps ? `<span>·</span><span class="mono">${esc(a.rateLimitRps)} rps</span>` : ''}
                          </div>
                        </td>
                        <td><span class="${statusClass(a.status)}">${esc(a.status)}</span></td>
                        <td class="mono">${esc(a.internalPort || '—')}</td>
                        <td class="right">
                          <div class="row" style="justify-content:flex-end">
                            <button type="button" class="btn btn-sm ${a.sslEnabled ? '' : 'btn-primary'}" data-app-ssl="${esc(a.id)}">SSL / HTTPS</button>
                            <button type="button" class="btn btn-sm" data-app-vhost="${esc(a.id)}">Vhost &amp; Log</button>
                            <button type="button" class="btn btn-sm" data-app-settings="${esc(a.id)}">Cài đặt Site</button>
                            <button type="button" class="btn btn-sm" data-app-act="restart" data-id="${esc(a.id)}">Restart</button>
                            <button type="button" class="btn btn-sm" data-app-job="rebuild" data-id="${esc(a.id)}">Rebuild</button>
                            ${a.gitRepoUrl ? `<button type="button" class="btn btn-sm" data-app-job="git-deploy" data-id="${esc(a.id)}">Git Pull</button>` : ''}
                            <button type="button" class="btn btn-sm" data-app-probe="${esc(a.id)}">Test HTTP</button>
                            <button type="button" class="btn btn-sm btn-danger" data-app-del="${esc(a.id)}" data-domain="${esc(a.domain)}">Xóa</button>
                          </div>
                        </td>
                      </tr>`).join('')
                    : '<tr><td colspan="5" class="empty-state">Chưa có website nào. Nhấn "Thêm Website mới" để triển khai.</td></tr>'
                }
              </tbody>
            </table>
          </div>
        </section>
      ` : `
        <div class="grid-3">
          ${
            templates.length
              ? templates.map((t) => `
                <div class="surface">
                  <div class="surface-head">
                    <h3 class="surface-title">${esc(t.name)}</h3>
                    <span class="meta-line mono">${esc(t.version || '')}</span>
                  </div>
                  <p class="page-desc" style="margin-bottom:12px">${esc(t.description || '')}</p>
                  <div class="meta-line" style="margin-bottom:14px">
                    <span>${esc(t.category || 'App')}</span>
                    <span>·</span>
                    <span>${esc(t.requiresDb && t.requiresDb !== 'None' ? `CSDL: ${t.requiresDb}` : (t.popularScore || t.framework || 'Static'))}</span>
                  </div>
                  <button type="button" class="btn btn-primary btn-sm" data-oc-deploy="${esc(t.id)}" data-oc-name="${esc(t.name)}" data-oc-db="${esc(t.requiresDb || 'None')}">Triển khai ngay</button>
                </div>`).join('')
              : '<div class="surface"><div class="empty-state">Không có mẫu 1-Click nào khả dụng.</div></div>'
          }
        </div>
      `}
    `;

    document.getElementById('tab-sites')?.addEventListener('click', () => { activeTab = 'sites'; draw(); });
    document.getElementById('tab-oneclick')?.addEventListener('click', () => { activeTab = 'oneclick'; draw(); });
    document.getElementById('apps-search')?.addEventListener('input', (e) => { searchTerm = e.target.value; draw(); });
    document.getElementById('btn-deploy-app')?.addEventListener('click', () => openDeployModal(container));

    container.querySelectorAll('[data-app-ssl]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const app = apps.find((x) => x.id === btn.dataset.appSsl);
        if (app) openSslManagerModal(app, container);
      });
    });

    container.querySelectorAll('[data-app-vhost]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const app = apps.find((x) => x.id === btn.dataset.appVhost);
        if (app) openVhostLogsModal(app, container);
      });
    });

    container.querySelectorAll('[data-app-settings]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const app = apps.find((x) => x.id === btn.dataset.appSettings);
        if (app) openSiteSettingsModal(app, container);
      });
    });

    container.querySelectorAll('[data-app-act]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          await api('POST', `/api/apps/${encodeURIComponent(btn.dataset.id)}/action`, { action: btn.dataset.appAct });
          toast('Đã khởi động lại dịch vụ và xác nhận trạng thái active.');
          await renderApps(container);
        } catch (err) {
          toast(err.message, 'err');
        }
      }));
    });

    container.querySelectorAll('[data-app-job]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          const job = await api('POST', `/api/jobs/apps/${encodeURIComponent(btn.dataset.id)}/${btn.dataset.appJob}`, {});
          toast(`Đã xếp hàng tác vụ ${job.title}`);
          openJobDrawer(job.id);
        } catch (err) {
          toast(err.message, 'err');
        }
      }));
    });

    container.querySelectorAll('[data-app-probe]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          const res = await api('POST', `/api/apps/${encodeURIComponent(btn.dataset.appProbe)}/test-http`, {});
          toast(`HTTP ${res.statusCode || 200} · ${res.latencyMs || 0} ms · ${res.protocol || 'HTTP/1.1'}`);
        } catch (err) {
          toast(err.message, 'err');
        }
      }));
    });

    container.querySelectorAll('[data-app-del]').forEach((btn) => {
      btn.addEventListener('click', () => {
        confirmAction('Xóa Website', `Xác nhận xóa vĩnh viễn website ${btn.dataset.domain} cùng cấu hình vhost và unit systemd?`, async () => {
          await withBusy(btn, async () => {
            try {
              await api('DELETE', `/api/apps/${encodeURIComponent(btn.dataset.appDel)}`);
              toast(`Đã xóa ${btn.dataset.domain}`);
              await renderApps(container);
            } catch (err) {
              toast(err.message, 'err');
            }
          });
        });
      });
    });

    container.querySelectorAll('[data-oc-deploy]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const requiresDb = btn.dataset.ocDb && btn.dataset.ocDb !== 'None' ? btn.dataset.ocDb : '';
        openModal(`Triển khai 1-Click: ${btn.dataset.ocName}`, `
          <form id="oc-form">
            <div id="oc-form-err"></div>
            <div class="field">
              <label for="oc-domain">Tên miền triển khai</label>
              <input class="input" id="oc-domain" name="domain" inputmode="url" autocapitalize="none" autocorrect="off" spellcheck="false" placeholder="blog.example.com" required />
            </div>
            <div class="field">
              <label for="oc-title">Tiêu đề trang</label>
              <input class="input" id="oc-title" name="siteTitle" value="${esc(btn.dataset.ocName)}" />
            </div>
            <div class="field">
              <label for="oc-email">Email quản trị (dùng cho SSL / Admin)</label>
              <input class="input" id="oc-email" name="adminEmail" type="email" inputmode="email" autocapitalize="none" placeholder="admin@example.com" />
            </div>
            <div class="field">
              <label for="oc-source-url">URL nguồn tài nguyên bên ngoài (tùy chọn: .git / .tar.gz / .zip)</label>
              <input class="input" id="oc-source-url" name="sourceUrl" inputmode="url" autocapitalize="none" autocorrect="off" spellcheck="false" placeholder="https://... (để trống để dùng nguồn chuẩn của mẫu)" />
            </div>
            ${
              requiresDb
                ? `<div class="field">
                    <label class="checkbox-row">
                      <input type="checkbox" name="createDb" checked />
                      <span>Tự động tạo cơ sở dữ liệu (${esc(requiresDb)}) &amp; cấu hình kết nối</span>
                    </label>
                  </div>`
                : ''
            }
            <div class="field">
              <label class="checkbox-row">
                <input type="checkbox" name="autoInstallDeps" checked />
                <span>Tự động nạp tài nguyên &amp; runtime từ bên ngoài (PHP-FPM, MariaDB/PostgreSQL, Bun, Tailwind, Cargo, Git nếu máy chủ chưa cài)</span>
              </label>
            </div>
            <div class="field">
              <label class="checkbox-row">
                <input type="checkbox" name="enableSsl" />
                <span>Bật HTTPS / SSL ngay (yêu cầu tên miền đã trỏ DNS về máy chủ)</span>
              </label>
            </div>
            <div id="oc-progress-note" class="muted" style="font-size:12px;margin-top:8px;display:none"></div>
            <div class="row" style="justify-content:flex-end;margin-top:16px">
              <button type="submit" class="btn btn-primary" id="oc-submit-btn">Bắt đầu triển khai</button>
            </div>
          </form>
        `, {
          onMount(body) {
            body.querySelector('#oc-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              const sub = body.querySelector('#oc-submit-btn');
              const errBox = body.querySelector('#oc-form-err');
              const progNote = body.querySelector('#oc-progress-note');
              if (errBox) {
                errBox.className = '';
                errBox.textContent = '';
              }
              await withBusy(sub, async () => {
                const fd = new FormData(e.target);
                const domain = String(fd.get('domain') || '').trim();
                const autoInstallDeps = fd.get('autoInstallDeps') === 'on';
                const sourceUrl = String(fd.get('sourceUrl') || '').trim();
                if (progNote) {
                  progNote.style.display = 'block';
                  progNote.textContent = autoInstallDeps
                    ? 'Đang tự động nạp tài nguyên/runtime từ bên ngoài và triển khai (có thể mất 30–90 giây ở lần đầu cài PHP/DB)...'
                    : 'Đang triển khai ứng dụng...';
                }
                try {
                  await api('POST', '/api/oneclick/deploy', {
                    templateId: btn.dataset.ocDeploy,
                    domain,
                    siteTitle: String(fd.get('siteTitle') || '').trim(),
                    adminEmail: String(fd.get('adminEmail') || '').trim(),
                    enableSsl: fd.get('enableSsl') === 'on',
                    createDb: requiresDb ? fd.get('createDb') === 'on' : false,
                    autoInstallDeps,
                    sourceUrl: sourceUrl || undefined,
                  });
                  closeModal();
                  toast(`Đã triển khai 1-Click ${btn.dataset.ocName} cho ${domain}.`);
                  await renderApps(container);
                } catch (err) {
                  if (progNote) progNote.style.display = 'none';
                  if (errBox) {
                    errBox.className = 'alert-err';
                    errBox.textContent = err.message || 'Triển khai 1-Click thất bại.';
                  }
                  toast(err.message, 'err');
                }
              });
            });
          },
        });
      });
    });
  }

  draw();
}

function openSslManagerModal(app, container) {
  let activeTab = 'letsencrypt';
  let info = null;

  openModal(`Quản lý SSL / HTTPS: ${app.domain}`, '<div class="skel skel-lg" style="margin:12px 0"></div><div class="skel skel-row"></div>', {
    large: true,
    onMount(body) {
      async function refresh() {
        try {
          info = await api('GET', `/api/apps/${encodeURIComponent(app.id)}/ssl`);
          paint();
        } catch (err) {
          body.innerHTML = `<div class="alert-err">${esc(err.message || 'Không tải được trạng thái SSL.')}</div>`;
        }
      }

      function paint() {
        const cert = info.cert || {};
        const timer = info.renewalTimer || {};
        const dnsList = Array.isArray(info.dnsPreflight) ? info.dnsPreflight : [];
        const certStateLabel = {
          valid: 'Hợp lệ',
          expiring_soon: 'Sắp hết hạn',
          expired: 'Đã hết hạn',
          missing: 'Chưa cài',
        }[cert.status] || cert.status || 'missing';
        const certStateCls = cert.status === 'valid'
          ? 'status-ok'
          : (cert.status === 'expiring_soon' ? 'status-warn' : (cert.status === 'expired' ? 'status-err' : 'muted'));

        body.innerHTML = `
          <div id="ssl-feedback"></div>

          <div class="kv-grid" style="margin-bottom:14px">
            <div class="kv-item">
              <span class="kv-label">Trạng thái HTTPS (${esc(info.webServerEngine || 'nginx')})</span>
              <span class="kv-value">
                <strong class="${info.sslEnabled ? 'status-ok' : 'muted'}">${info.sslEnabled ? 'HTTPS Đang bật' : 'Chỉ HTTP (80)'}</strong>
                ${info.sslEnabled ? ` · <span class="mono">${info.forceHttps ? 'Force 301' : 'HTTP+HTTPS'}</span>` : ''}
              </span>
            </div>
            <div class="kv-item">
              <span class="kv-label">Chứng chỉ X.509</span>
              <span class="kv-value">
                <strong class="${certStateCls}">${esc(certStateLabel)}</strong>
                ${cert.provider ? ` · ${esc(cert.provider)}` : ''}
              </span>
            </div>
            <div class="kv-item">
              <span class="kv-label">Hạn sử dụng</span>
              <span class="kv-value mono">
                ${cert.present && cert.notAfter ? `${esc(cert.notAfter.slice(0, 10))} (${esc(cert.daysRemaining ?? 0)} ngày)` : '—'}
              </span>
            </div>
            <div class="kv-item">
              <span class="kv-label">Tự động gia hạn (systemd)</span>
              <span class="kv-value mono">
                <span class="${timer.active ? 'status-ok' : 'muted'}">${timer.active ? 'Đang chạy' : 'Chưa bật'}</span>
                ${timer.nextRun ? ` · ${esc(timer.nextRun)}` : ''}
              </span>
            </div>
          </div>

          <div class="table-wrap" style="margin-bottom:14px">
            <table class="data-table">
              <thead><tr><th>Kiểm tra DNS Preflight</th><th>IP phân giải</th><th>Trạng thái ACME HTTP-01</th></tr></thead>
              <tbody>
                ${dnsList.map((d) => `
                  <tr>
                    <td class="mono"><strong>${esc(d.domain)}</strong></td>
                    <td class="mono">${d.ips && d.ips.length ? esc(d.ips.join(', ')) : '—'}</td>
                    <td>${d.resolved
                      ? '<span class="status-ok">Sẵn sàng cấp Let\'s Encrypt</span>'
                      : `<span class="status-err">${esc(d.warning || 'Chưa phân giải DNS')}</span>`}</td>
                  </tr>
                `).join('')}
              </tbody>
            </table>
          </div>

          ${cert.present ? `
            <div class="meta-line" style="margin-bottom:14px;flex-wrap:wrap">
              <span>Issuer: <strong class="mono">${esc(cert.issuer || '—')}</strong></span>
              <span>·</span>
              <span>SANs: <strong class="mono">${esc((cert.sanDomains && cert.sanDomains.length ? cert.sanDomains : [info.domain]).join(', '))}</strong></span>
              ${cert.sha256Fingerprint ? `<span>·</span><span class="mono" title="${esc(cert.sha256Fingerprint)}">SHA-256: ${esc(cert.sha256Fingerprint.slice(0, 26))}…</span>` : ''}
            </div>
          ` : ''}

          <div class="tabs" style="margin-bottom:14px">
            <button type="button" class="tab-btn ${activeTab === 'letsencrypt' ? 'active' : ''}" data-ssl-tab="letsencrypt">Let's Encrypt (Miễn phí)</button>
            <button type="button" class="tab-btn ${activeTab === 'custom' ? 'active' : ''}" data-ssl-tab="custom">Chứng chỉ PEM / Cloudflare</button>
            <button type="button" class="tab-btn ${activeTab === 'selfsigned' ? 'active' : ''}" data-ssl-tab="selfsigned">Self-Signed (OpenSSL)</button>
            <button type="button" class="tab-btn ${activeTab === 'manage' ? 'active' : ''}" data-ssl-tab="manage">Cài đặt &amp; Gỡ SSL</button>
          </div>

          ${activeTab === 'letsencrypt' ? `
            <form id="ssl-le-form">
              <div class="form-grid">
                <div class="field">
                  <label>Email thông báo ACME (tùy chọn)</label>
                  <input class="input" name="email" type="email" inputmode="email" autocapitalize="none" placeholder="admin@${esc(info.domain)}" />
                </div>
                <div class="field">
                  <label>Tên miền phụ / SAN bổ sung (phân cách dấu phẩy)</label>
                  <input class="input mono" name="aliases" value="${esc((info.aliases || []).join(', '))}" placeholder="www.${esc(info.domain)}" />
                </div>
              </div>
              <div class="row" style="margin:8px 0 12px;gap:14px">
                <label class="checkbox-row">
                  <input type="checkbox" name="forceHttps" ${info.sslEnabled ? (info.forceHttps ? 'checked' : '') : 'checked'} />
                  <span>Ép chuyển hướng HTTP → HTTPS (301)</span>
                </label>
                <label class="checkbox-row">
                  <input type="checkbox" name="autoInstallCertbot" ${info.certbotInstalled ? '' : 'checked'} />
                  <span>Tự động cài Certbot (apt-get) nếu thiếu ${info.certbotInstalled ? '(đã cài)' : '(chưa cài)'}</span>
                </label>
                <label class="checkbox-row">
                  <input type="checkbox" name="forceRenewal" />
                  <span>Ép cấp lại ngay (--force-renewal)</span>
                </label>
              </div>
              <p class="page-desc" style="margin-bottom:12px">
                Xác thực thực qua webroot <code class="mono">${esc(info.acmeWebroot || '/www/acme')}/.well-known/acme-challenge/</code> trên cổng 80, tự động bật <code>ferrous-certbot-renew.timer</code> và tải lại ${esc(info.webServerEngine || 'nginx')}.
              </p>
              <div class="row" style="justify-content:flex-end">
                <button type="submit" class="btn btn-primary" id="ssl-le-submit">Cấp / Gia hạn Let's Encrypt</button>
              </div>
            </form>
          ` : ''}

          ${activeTab === 'custom' ? `
            <form id="ssl-custom-form">
              <div class="field">
                <div class="row" style="justify-content:space-between;margin-bottom:6px">
                  <label style="margin:0">Certificate / Fullchain PEM (<code>.pem</code> / <code>.crt</code>)</label>
                  <label class="btn btn-sm" style="cursor:pointer">Nạp từ tệp<input type="file" id="ssl-cert-file" accept=".pem,.crt,.cer,.txt" hidden /></label>
                </div>
                <textarea class="textarea mono" name="fullchainPem" id="ssl-cert-ta" style="min-height:110px" placeholder="-----BEGIN CERTIFICATE-----&#10;...&#10;-----END CERTIFICATE-----" required></textarea>
              </div>
              <div class="field">
                <div class="row" style="justify-content:space-between;margin-bottom:6px">
                  <label style="margin:0">Private Key PEM (<code>.key</code> / <code>.pem</code> — lưu quyền 0600, không ghi log)</label>
                  <label class="btn btn-sm" style="cursor:pointer">Nạp từ tệp<input type="file" id="ssl-key-file" accept=".key,.pem,.txt" hidden /></label>
                </div>
                <textarea class="textarea mono" name="privateKeyPem" id="ssl-key-ta" style="min-height:110px" placeholder="-----BEGIN PRIVATE KEY-----&#10;...&#10;-----END PRIVATE KEY-----" required></textarea>
              </div>
              <div class="row" style="justify-content:space-between;margin-top:10px">
                <label class="checkbox-row">
                  <input type="checkbox" name="forceHttps" checked />
                  <span>Ép chuyển hướng HTTP → HTTPS (301)</span>
                </label>
                <button type="submit" class="btn btn-primary" id="ssl-custom-submit">Kiểm tra khóa &amp; Cài chứng chỉ</button>
              </div>
            </form>
          ` : ''}

          ${activeTab === 'selfsigned' ? `
            <form id="ssl-self-form">
              <p class="page-desc" style="margin-bottom:12px">
                Tạo chứng chỉ X.509 v3 RSA-2048 SHA-256 kèm SANs trực tiếp bằng <code>openssl req</code>. Thích hợp cho môi trường nội bộ, kiểm thử hoặc <strong>Cloudflare SSL Mode = Full</strong>.
              </p>
              <div class="form-grid">
                <div class="field">
                  <label>Thời hạn hiệu lực</label>
                  <select class="select" name="validityDays">
                    <option value="365" selected>365 ngày (1 năm)</option>
                    <option value="730">730 ngày (2 năm)</option>
                    <option value="3650">3650 ngày (10 năm)</option>
                  </select>
                </div>
                <div class="field" style="display:flex;align-items:flex-end">
                  <label class="checkbox-row" style="margin-bottom:8px">
                    <input type="checkbox" name="forceHttps" />
                    <span>Ép chuyển hướng HTTP → HTTPS (301)</span>
                  </label>
                </div>
              </div>
              <div class="row" style="justify-content:flex-end;margin-top:12px">
                <button type="submit" class="btn btn-primary" id="ssl-self-submit">Tạo &amp; Áp dụng Self-Signed SSL</button>
              </div>
            </form>
          ` : ''}

          ${activeTab === 'manage' ? `
            <div class="row" style="justify-content:space-between;padding:10px 0;border-bottom:1px solid var(--border)">
              <div>
                <strong>Ép chuyển hướng HTTPS (HTTP 301)</strong>
                <div class="meta-line">Tự động chuyển mọi truy cập cổng 80 sang HTTPS (trừ đường dẫn ACME challenge).</div>
              </div>
              <button type="button" class="btn btn-sm" id="ssl-toggle-force" ${info.sslEnabled ? '' : 'disabled'}>
                ${info.forceHttps ? 'Tắt Force HTTPS' : 'Bật Force HTTPS (301)'}
              </button>
            </div>
            <div class="row" style="justify-content:space-between;padding:10px 0;border-bottom:1px solid var(--border)">
              <div>
                <strong>Bộ hẹn giờ gia hạn tự động (systemd)</strong>
                <div class="meta-line">Cài đặt <code>ferrous-certbot-renew.timer</code> chạy 2 lần/ngày (00:00 &amp; 12:00) kèm reload hook.</div>
              </div>
              <div class="row">
                <button type="button" class="btn btn-sm" id="ssl-enable-timer">Bật Auto-Renew Timer</button>
                <button type="button" class="btn btn-sm" id="ssl-bg-renew" ${info.certbotInstalled ? '' : 'disabled'}>Gia hạn nền (Job)</button>
              </div>
            </div>
            <div class="row" style="justify-content:space-between;padding:10px 0">
              <div>
                <strong>Tắt HTTPS / Gỡ chứng chỉ</strong>
                <div class="meta-line">Đưa vhost về chỉ phục vụ HTTP (cổng 80) và kiểm chứng cấu hình trước khi tải lại.</div>
              </div>
              <div class="row">
                <button type="button" class="btn btn-sm" id="ssl-disable-only" ${info.sslEnabled ? '' : 'disabled'}>Tắt HTTPS (giữ tệp)</button>
                <button type="button" class="btn btn-sm btn-danger" id="ssl-disable-del" ${info.sslEnabled || cert.present ? '' : 'disabled'}>Tắt &amp; Xóa chứng chỉ</button>
              </div>
            </div>
          ` : ''}
        `;

        const showErr = (msg) => {
          const fb = body.querySelector('#ssl-feedback');
          if (fb) {
            fb.className = 'alert-err';
            fb.textContent = msg;
          }
          toast(msg, 'err');
        };
        const showOk = (msg) => {
          const fb = body.querySelector('#ssl-feedback');
          if (fb) {
            fb.className = '';
            fb.textContent = '';
          }
          toast(msg);
        };

        body.querySelectorAll('[data-ssl-tab]').forEach((b) => {
          b.addEventListener('click', () => {
            activeTab = b.dataset.sslTab;
            paint();
          });
        });

        body.querySelector('#ssl-cert-file')?.addEventListener('change', (ev) => {
          const f = ev.target.files?.[0];
          if (!f) return;
          const reader = new FileReader();
          reader.onload = () => {
            const ta = body.querySelector('#ssl-cert-ta');
            if (ta) ta.value = String(reader.result || '');
          };
          reader.readAsText(f);
        });
        body.querySelector('#ssl-key-file')?.addEventListener('change', (ev) => {
          const f = ev.target.files?.[0];
          if (!f) return;
          const reader = new FileReader();
          reader.onload = () => {
            const ta = body.querySelector('#ssl-key-ta');
            if (ta) ta.value = String(reader.result || '');
          };
          reader.readAsText(f);
        });

        body.querySelector('#ssl-le-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const btn = body.querySelector('#ssl-le-submit');
          await withBusy(btn, async () => {
            const fd = new FormData(e.target);
            const aliases = String(fd.get('aliases') || '')
              .split(',')
              .map((s) => s.trim())
              .filter(Boolean);
            try {
              info = await api('POST', `/api/apps/${encodeURIComponent(app.id)}/ssl/letsencrypt`, {
                email: String(fd.get('email') || '').trim() || undefined,
                aliases,
                forceHttps: fd.get('forceHttps') === 'on',
                autoInstallCertbot: fd.get('autoInstallCertbot') === 'on',
                forceRenewal: fd.get('forceRenewal') === 'on',
              });
              showOk(`Đã cấp & kích hoạt SSL Let's Encrypt thật cho ${info.domain}.`);
              paint();
              await renderApps(container);
            } catch (err) {
              showErr(err.message || 'Cấp SSL Let\'s Encrypt thất bại.');
            }
          });
        });

        body.querySelector('#ssl-custom-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const btn = body.querySelector('#ssl-custom-submit');
          await withBusy(btn, async () => {
            const fd = new FormData(e.target);
            try {
              info = await api('POST', `/api/apps/${encodeURIComponent(app.id)}/ssl/custom`, {
                fullchainPem: String(fd.get('fullchainPem') || ''),
                privateKeyPem: String(fd.get('privateKeyPem') || ''),
                forceHttps: fd.get('forceHttps') === 'on',
              });
              showOk(`Đã xác minh khớp cặp khóa & cài chứng chỉ SSL cho ${info.domain}.`);
              paint();
              await renderApps(container);
            } catch (err) {
              showErr(err.message || 'Cài chứng chỉ PEM thất bại.');
            }
          });
        });

        body.querySelector('#ssl-self-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          const btn = body.querySelector('#ssl-self-submit');
          await withBusy(btn, async () => {
            const fd = new FormData(e.target);
            try {
              info = await api('POST', `/api/apps/${encodeURIComponent(app.id)}/ssl/self-signed`, {
                validityDays: Number(fd.get('validityDays') || 365),
                forceHttps: fd.get('forceHttps') === 'on',
              });
              showOk(`Đã tạo & áp dụng chứng chỉ Self-Signed cho ${info.domain}.`);
              paint();
              await renderApps(container);
            } catch (err) {
              showErr(err.message || 'Tạo chứng chỉ Self-Signed thất bại.');
            }
          });
        });

        body.querySelector('#ssl-toggle-force')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            info = await api('PUT', `/api/apps/${encodeURIComponent(app.id)}/ssl/settings`, {
              forceHttps: !info.forceHttps,
            });
            showOk(info.forceHttps ? 'Đã bật chuyển hướng HTTPS (301).' : 'Đã tắt ép chuyển hướng HTTPS.');
            paint();
            await renderApps(container);
          } catch (err) { showErr(err.message); }
        }));

        body.querySelector('#ssl-enable-timer')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            info = await api('PUT', `/api/apps/${encodeURIComponent(app.id)}/ssl/settings`, {
              enableAutoRenew: true,
            });
            showOk('Đã kích hoạt ferrous-certbot-renew.timer tự động gia hạn 2 lần/ngày.');
            paint();
          } catch (err) { showErr(err.message); }
        }));

        body.querySelector('#ssl-bg-renew')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            const job = await api('POST', `/api/jobs/apps/${encodeURIComponent(app.id)}/renew-ssl`, {});
            closeModal();
            toast(`Đã xếp hàng tác vụ ${job.title}`);
            openJobDrawer(job.id);
          } catch (err) { showErr(err.message); }
        }));

        body.querySelector('#ssl-disable-only')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            info = await api('DELETE', `/api/apps/${encodeURIComponent(app.id)}/ssl?deleteFiles=false`);
            showOk(`Đã tắt HTTPS trên vhost ${info.domain} (tệp chứng chỉ vẫn được giữ lại).`);
            paint();
            await renderApps(container);
          } catch (err) { showErr(err.message); }
        }));

        body.querySelector('#ssl-disable-del')?.addEventListener('click', () => {
          confirmAction('Tắt HTTPS & Xóa chứng chỉ', `Xác nhận tắt HTTPS và xóa tệp fullchain.pem / privkey.pem của ${app.domain}?`, async () => {
            try {
              await api('DELETE', `/api/apps/${encodeURIComponent(app.id)}/ssl?deleteFiles=true`);
              toast(`Đã tắt HTTPS và xóa chứng chỉ của ${app.domain}.`);
              await renderApps(container);
              openSslManagerModal(app, container);
            } catch (err) { toast(err.message, 'err'); }
          });
        });
      }

      refresh();
    },
  });
}

function openVhostLogsModal(app, container) {
  let activeTab = 'vhost';
  let vhostData = null;
  let logKind = 'error';
  let logLines = 150;
  let logData = null;

  openModal(`Vhost & Nhật ký: ${app.domain}`, '<div class="skel skel-lg" style="margin:12px 0"></div><div class="skel skel-row"></div>', {
    large: true,
    onMount(body) {
      async function loadVhost() {
        vhostData = await api('GET', `/api/apps/${encodeURIComponent(app.id)}/vhost`);
      }

      async function loadLogs() {
        logData = await api(
          'GET',
          `/api/apps/${encodeURIComponent(app.id)}/logs?kind=${encodeURIComponent(logKind)}&lines=${logLines}`,
        );
      }

      async function initLoad() {
        try {
          await loadVhost();
          paint();
        } catch (err) {
          body.innerHTML = `<div class="alert-err">${esc(err.message || 'Không tải được cấu hình Vhost.')}</div>`;
        }
      }

      function paint() {
        body.innerHTML = `
          <div id="vh-feedback"></div>
          <div class="tabs" style="margin-bottom:14px">
            <button type="button" class="tab-btn ${activeTab === 'vhost' ? 'active' : ''}" data-vh-tab="vhost">Cấu hình Vhost trực tiếp</button>
            <button type="button" class="tab-btn ${activeTab === 'logs' ? 'active' : ''}" data-vh-tab="logs">Nhật ký Website (Error / Access / Journal)</button>
          </div>

          ${activeTab === 'vhost' ? `
            <div class="meta-line" style="margin-bottom:10px;flex-wrap:wrap">
              <span>Engine: <strong class="mono">${esc(vhostData?.engine || 'nginx')}</strong></span>
              <span>·</span>
              <span>Tệp: <strong class="mono">${esc(vhostData?.configPath || '')}</strong></span>
              <span>·</span>
              <span class="${vhostData?.exists ? 'status-ok' : 'status-warn'}">${vhostData?.exists ? 'Đang có trên đĩa' : 'Chưa ghi trên đĩa'}</span>
            </div>
            <p class="page-desc" style="margin-bottom:10px">
              Khi lưu, hệ thống sẽ sao lưu snapshot tệp cũ, ghi atomic, chạy kiểm tra cú pháp thật (<span class="mono">nginx -t</span> / <span class="mono">ferrousx-bin --check-config</span>), tự động khôi phục nếu sai cú pháp và reload dịch vụ.
            </p>
            <div class="field">
              <textarea class="textarea mono" id="vh-editor" style="min-height:340px;font-size:12px;line-height:1.5" spellcheck="false">${esc(vhostData?.content || '')}</textarea>
            </div>
            <div class="row" style="justify-content:flex-end;margin-top:12px">
              <button type="button" class="btn" id="vh-reload-btn">Đọc lại từ đĩa</button>
              <button type="button" class="btn btn-primary" id="vh-save-btn">Kiểm tra cú pháp &amp; Áp dụng (Reload)</button>
            </div>
          ` : `
            <div class="row-between" style="margin-bottom:10px;gap:10px;flex-wrap:wrap">
              <div class="row" style="gap:8px;flex-wrap:wrap">
                <select class="select" id="vh-log-kind" style="width:auto">
                  <option value="error" ${logKind === 'error' ? 'selected' : ''}>Nginx Error Log</option>
                  <option value="access" ${logKind === 'access' ? 'selected' : ''}>Nginx Access Log</option>
                  <option value="journal" ${logKind === 'journal' ? 'selected' : ''}>Systemd Journal (App / PHP-FPM)</option>
                </select>
                <select class="select" id="vh-log-lines" style="width:auto">
                  <option value="50" ${logLines === 50 ? 'selected' : ''}>50 dòng cuối</option>
                  <option value="150" ${logLines === 150 ? 'selected' : ''}>150 dòng cuối</option>
                  <option value="300" ${logLines === 300 ? 'selected' : ''}>300 dòng cuối</option>
                </select>
              </div>
              <div class="row" style="gap:8px">
                <button type="button" class="btn btn-sm" id="vh-log-refresh">Làm mới</button>
                ${logKind !== 'journal' ? '<button type="button" class="btn btn-sm btn-danger" id="vh-log-clear">Làm trống log</button>' : ''}
              </div>
            </div>
            <div class="meta-line" style="margin-bottom:10px;flex-wrap:wrap">
              <span>Nguồn: <strong class="mono">${esc(logData?.sourcePath || '—')}</strong></span>
              <span>·</span>
              <span>Dung lượng: <strong class="mono">${esc(fmtBytes(logData?.sizeBytes || 0))}</strong></span>
              ${logData?.isSharedLog ? '<span>·</span><span class="status-warn">Log dùng chung của hệ thống</span>' : ''}
            </div>
            <pre class="terminal-box" id="vh-log-output" style="min-height:320px;max-height:460px;white-space:pre-wrap">${esc(logData?.content || 'Đang tải nhật ký…')}</pre>
          `}
        `;

        const fb = body.querySelector('#vh-feedback');
        const showMsg = (msg, err = false) => {
          if (fb) fb.innerHTML = `<div class="${err ? 'alert-err' : 'alert-ok'}" style="margin-bottom:12px;white-space:pre-wrap">${esc(msg)}</div>`;
          toast(msg, err ? 'err' : 'ok');
        };

        body.querySelectorAll('[data-vh-tab]').forEach((b) => {
          b.addEventListener('click', async () => {
            activeTab = b.dataset.vhTab;
            if (activeTab === 'logs' && !logData) {
              paint();
              try {
                await loadLogs();
                paint();
              } catch (err) {
                showMsg(err.message || 'Không tải được nhật ký.', true);
              }
            } else {
              paint();
            }
          });
        });

        body.querySelector('#vh-reload-btn')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            await loadVhost();
            paint();
            toast('Đã tải lại cấu hình Vhost từ đĩa.');
          } catch (err) {
            showMsg(err.message, true);
          }
        }));

        body.querySelector('#vh-save-btn')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          const content = body.querySelector('#vh-editor')?.value || '';
          try {
            const res = await api('PUT', `/api/apps/${encodeURIComponent(app.id)}/vhost`, { content });
            vhostData = { ...(vhostData || {}), exists: true, content, configPath: res.configPath };
            showMsg(`Đã kiểm tra cú pháp & reload ${res.engine} thành công (${res.configPath}).`);
            await renderApps(container);
          } catch (err) {
            showMsg(err.message || 'Lưu cấu hình Vhost thất bại.', true);
          }
        }));

        body.querySelector('#vh-log-kind')?.addEventListener('change', async (e) => {
          logKind = e.target.value;
          try {
            await loadLogs();
            paint();
          } catch (err) {
            showMsg(err.message, true);
          }
        });

        body.querySelector('#vh-log-lines')?.addEventListener('change', async (e) => {
          logLines = Number(e.target.value || 150);
          try {
            await loadLogs();
            paint();
          } catch (err) {
            showMsg(err.message, true);
          }
        });

        body.querySelector('#vh-log-refresh')?.addEventListener('click', (e) => withBusy(e.currentTarget, async () => {
          try {
            await loadLogs();
            paint();
          } catch (err) {
            showMsg(err.message, true);
          }
        }));

        body.querySelector('#vh-log-clear')?.addEventListener('click', (e) => {
          const btn = e.currentTarget;
          confirmAction(
            'Làm trống tệp nhật ký',
            `Xác nhận làm trống tệp nhật ký ${logData?.sourcePath || logKind} (0 byte)?`,
            () => withBusy(btn, async () => {
              try {
                logData = await api(
                  'DELETE',
                  `/api/apps/${encodeURIComponent(app.id)}/logs?kind=${encodeURIComponent(logKind)}`,
                );
                paint();
                toast('Đã làm trống tệp nhật ký.');
              } catch (err) {
                showMsg(err.message, true);
              }
            }),
          );
        });
      }

      initLoad();
    },
  });
}

function openSiteSettingsModal(app, container) {
  const hotlink = app.hotlinkProtection || {
    enabled: false,
    protectedExtensions: 'jpg,jpeg,png,gif,webp,mp4',
    allowedDomains: [],
    blockEmptyReferer: false,
  };
  const redirectsText = (app.redirects || [])
    .filter((r) => r.enabled !== false)
    .map((r) => `${r.sourcePath || r.source_path} -> ${r.targetUrl || r.target_url} ${r.type || r.statusCode || r.status_code || '301'}`)
    .join('\n');
  const firstAuth = (app.directoryAuth || app.directoryPrivacy || [])[0] || null;

  openModal(`Cài đặt nâng cao: ${app.domain}`, `
    <form id="site-settings-form">
      <div class="row" style="margin-bottom:12px">
        <label class="checkbox-row"><input type="checkbox" name="sslEnabled" ${app.sslEnabled ? 'checked' : ''} /><span>Bật HTTPS / SSL</span></label>
        <label class="checkbox-row"><input type="checkbox" name="forceHttps" ${app.forceHttps ? 'checked' : ''} /><span>Ép chuyển hướng HTTPS (301)</span></label>
        <label class="checkbox-row"><input type="checkbox" name="websocketEnabled" ${app.websocketEnabled !== false ? 'checked' : ''} /><span>Bật WebSocket Proxy</span></label>
        <label class="checkbox-row"><input type="checkbox" name="http3Quic" ${app.http3Quic ? 'checked' : ''} /><span>Bật HTTP/3 QUIC (chỉ khi engine hỗ trợ)</span></label>
      </div>
      <div class="form-grid">
        <div class="field">
          <label>Rate Limit theo IP (req/giây, 0 = tắt)</label>
          <input class="input mono" name="rateLimitRps" type="number" min="0" max="10000" value="${esc(app.rateLimitRps || 0)}" />
        </div>
        <div class="field">
          <label>Chống Hotlink — Tên miền cho phép (phân cách bằng dấu phẩy)</label>
          <input class="input mono" name="hotlinkDomains" value="${esc((hotlink.allowedDomains || []).join(', '))}" placeholder="example.com, cdn.example.com" />
        </div>
        <div class="field">
          <label>Phần mở rộng bảo vệ Hotlink</label>
          <input class="input mono" name="hotlinkExtensions" value="${esc(hotlink.protectedExtensions || 'jpg,jpeg,png,gif,webp,mp4')}" placeholder="jpg,jpeg,png,gif,webp,mp4" />
        </div>
      </div>
      <div class="row" style="margin-bottom:12px">
        <label class="checkbox-row"><input type="checkbox" name="hotlinkEnabled" ${hotlink.enabled ? 'checked' : ''} /><span>Bật chống Hotlink ảnh/video</span></label>
        <label class="checkbox-row"><input type="checkbox" name="hotlinkBlockEmpty" ${hotlink.blockEmptyReferer ? 'checked' : ''} /><span>Chặn truy cập không có Referer</span></label>
      </div>
      <div class="field">
        <label>Chuyển hướng URL (mỗi dòng: <code>/old-path -> https://example.com/new 301</code>)</label>
        <textarea class="textarea" name="redirects" style="min-height:80px" placeholder="/blog -> https://blog.example.com 301">${esc(redirectsText)}</textarea>
      </div>
      <hr class="divider" />
      <div class="meta-line" style="margin-bottom:8px"><strong>Khóa thư mục bằng HTTP Basic Auth (.htpasswd $apr1$)</strong></div>
      <div class="form-grid">
        <div class="field">
          <label>Đường dẫn bảo vệ (để trống để tắt)</label>
          <input class="input mono" name="authPath" value="${esc(firstAuth?.path || '')}" placeholder="/admin" />
        </div>
        <div class="field">
          <label>Nhãn / Realm</label>
          <input class="input" name="authLabel" value="${esc(firstAuth?.label || firstAuth?.realm || 'Protected Area')}" />
        </div>
        <div class="field">
          <label>Tài khoản</label>
          <input class="input mono" name="authUser" value="${esc(firstAuth?.username || '')}" placeholder="admin" />
        </div>
        <div class="field">
          <label>Mật khẩu mới (để trống để giữ hash cũ)</label>
          <input class="input" name="authPass" type="password" placeholder="••••••••" />
        </div>
      </div>
      <div class="row" style="justify-content:flex-end;margin-top:14px">
        <button type="submit" class="btn btn-primary" id="ss-submit-btn">Lưu & Áp dụng cấu hình</button>
      </div>
    </form>
  `, {
    large: true,
    onMount(body) {
      body.querySelector('#site-settings-form')?.addEventListener('submit', async (e) => {
        e.preventDefault();
        const sub = body.querySelector('#ss-submit-btn');
        await withBusy(sub, async () => {
          const fd = new FormData(e.target);
          const redirects = String(fd.get('redirects') || '')
            .split('\n')
            .map((l) => l.trim())
            .filter(Boolean)
            .map((line, idx) => {
              const parts = line.split('->').map((s) => s.trim());
              const src = parts[0] || '/';
              const rhs = (parts[1] || '').split(/\s+/);
              const targetUrl = rhs[0] || '/';
              const redirectType = rhs[1] === '302' ? '302' : '301';
              return { id: `redir-${idx + 1}`, sourcePath: src, targetUrl, type: redirectType, keepQuery: true, enabled: true };
            });

          const authPath = String(fd.get('authPath') || '').trim();
          const authUser = String(fd.get('authUser') || '').trim();
          const authPass = String(fd.get('authPass') || '');
          const directoryAuth = authPath && authUser
            ? [{
                id: firstAuth?.id || 'dir-auth-1',
                path: authPath,
                label: String(fd.get('authLabel') || 'Protected Area').trim() || 'Protected Area',
                username: authUser,
                enabled: true,
                ...(authPass ? { password: authPass } : {}),
              }]
            : [];

          const hotlinkDomains = String(fd.get('hotlinkDomains') || '')
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean);

          try {
            await api('PUT', `/api/apps/${encodeURIComponent(app.id)}/site-settings`, {
              sslEnabled: fd.get('sslEnabled') === 'on',
              forceHttps: fd.get('forceHttps') === 'on',
              http3Quic: fd.get('http3Quic') === 'on',
              websocketEnabled: fd.get('websocketEnabled') === 'on',
              rateLimitRps: Number(fd.get('rateLimitRps') || 0),
              redirects,
              hotlinkProtection: {
                enabled: fd.get('hotlinkEnabled') === 'on',
                protectedExtensions: String(fd.get('hotlinkExtensions') || 'jpg,jpeg,png,gif,webp,mp4').trim() || 'jpg,jpeg,png,gif,webp,mp4',
                allowedDomains: hotlinkDomains,
                blockEmptyReferer: fd.get('hotlinkBlockEmpty') === 'on',
              },
              directoryAuth,
            });
            closeModal();
            toast(`Đã cập nhật & kiểm chứng cấu hình vhost cho ${app.domain}.`);
            await renderApps(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      });
    },
  });
}

function openDeployModal(container) {
  openModal('Triển khai Website mới', `
    <form id="deploy-app-form">
      <div class="form-grid">
        <div class="field">
          <label>Tên hiển thị</label>
          <input class="input" name="name" placeholder="My Web App" required />
        </div>
        <div class="field">
          <label>Tên miền chính</label>
          <input class="input" name="domain" placeholder="app.example.com" required />
        </div>
        <div class="field">
          <label>Framework / Runtime</label>
          <select class="select" name="framework">
            ${RUNTIMES.map((r) => `<option value="${esc(r)}">${esc(r)}</option>`).join('')}
          </select>
        </div>
        <div class="field">
          <label>Cổng nội bộ (Reverse Proxy)</label>
          <input class="input mono" name="internalPort" type="number" value="8080" min="1024" max="65535" />
        </div>
        <div class="field">
          <label>Lệnh khởi chạy (tùy chọn)</label>
          <input class="input mono" name="execCommand" placeholder="bun run start" />
        </div>
        <div class="field">
          <label>Git Repository URL (tùy chọn)</label>
          <input class="input mono" name="gitRepoUrl" placeholder="https://github.com/org/repo.git" />
        </div>
      </div>
      <div class="row" style="margin:8px 0 14px">
        <label class="checkbox-row"><input type="checkbox" name="sslEnabled" /><span>Tự động cấp SSL Let's Encrypt</span></label>
        <label class="checkbox-row"><input type="checkbox" name="websocketEnabled" checked /><span>Hỗ trợ WebSocket</span></label>
        <label class="checkbox-row"><input type="checkbox" name="http3Quic" /><span>HTTP/3 QUIC</span></label>
      </div>
      <div class="row" style="justify-content:flex-end">
        <button type="submit" class="btn btn-primary" id="deploy-submit-btn">Tạo & Triển khai</button>
      </div>
    </form>
  `, {
    onMount(body) {
      body.querySelector('#deploy-app-form')?.addEventListener('submit', async (e) => {
        e.preventDefault();
        const sub = body.querySelector('#deploy-submit-btn');
        await withBusy(sub, async () => {
          const fd = new FormData(e.target);
          try {
            await api('POST', '/api/apps', {
              name: String(fd.get('name') || '').trim(),
              framework: String(fd.get('framework') || 'Static HTML / SPA'),
              domain: String(fd.get('domain') || '').trim(),
              aliases: [],
              internalPort: Number(fd.get('internalPort') || 8080),
              workingDir: '',
              runSubDir: '',
              execCommand: String(fd.get('execCommand') || '').trim(),
              workers: 1,
              memoryLimitMb: 256,
              sslEnabled: fd.get('sslEnabled') === 'on',
              http3Quic: fd.get('http3Quic') === 'on',
              websocketEnabled: fd.get('websocketEnabled') === 'on',
              rateLimitRps: 0,
              createDatabase: false,
              dbEngine: '',
              dbName: '',
              dbUser: '',
              dbPassword: '',
              createFtp: false,
              ftpUser: '',
              gitRepoUrl: String(fd.get('gitRepoUrl') || '').trim(),
              gitBranch: 'main',
            });
            closeModal();
            toast('Đã triển khai website mới.');
            await renderApps(container);
          } catch (err) {
            toast(err.message, 'err');
          }
        });
      });
    },
  });
}

// ============================================================================
// 3. SAFEPATH FILE MANAGER (FULL PARITY: RENAME, COMPRESS, REMOTE DOWNLOAD, CHMOD)
// ============================================================================
export async function renderFiles(container) {
  const apps = await api('GET', '/api/apps');
  if (!Array.isArray(apps) || !apps.length) {
    container.innerHTML = `
      <div class="page-head"><h1 class="page-title">Quản lý Tệp</h1></div>
      <div class="surface"><div class="empty-state">Cần tạo ít nhất 1 website trước khi duyệt tệp trong /www/wwwroot.</div></div>
    `;
    return;
  }

  let selectedAppId = apps[0].id;
  let currentDir = '/';
  let nodes = [];

  async function loadAndDraw() {
    nodes = await api('GET', `/api/files/${encodeURIComponent(selectedAppId)}`);
    const visible = (Array.isArray(nodes) ? nodes : []).filter((n) => (n.parentDir || '/') === currentDir);

    container.innerHTML = `
      <div class="page-head">
        <div>
          <h1 class="page-title">Quản lý Tệp (SafePath)</h1>
          <p class="page-desc">Chỉnh sửa mã nguồn, đổi tên, nén/giải nén an toàn chống symlink escape, tải từ URL chống SSRF và chuẩn hóa quyền.</p>
        </div>
        <div class="row">
          <select class="select" id="fm-app-select" style="width:auto;min-width:200px">
            ${apps.map((a) => `<option value="${esc(a.id)}" ${a.id === selectedAppId ? 'selected' : ''}>${esc(a.domain)}</option>`).join('')}
          </select>
          <button type="button" class="btn btn-sm" id="fm-new-file">Tạo tệp</button>
          <button type="button" class="btn btn-sm" id="fm-new-dir">Tạo thư mục</button>
          <button type="button" class="btn btn-sm" id="fm-compress-btn">Nén thư mục</button>
          <button type="button" class="btn btn-sm" id="fm-remote-dl-btn">Tải từ URL</button>
          <button type="button" class="btn btn-sm" id="fm-chmod-btn">Batch Chmod</button>
          <label class="btn btn-sm btn-primary" style="cursor:pointer">
            Tải tệp lên
            <input type="file" id="fm-upload-input" hidden />
          </label>
        </div>
      </div>

      ${capBannerHtml(['files'])}

      <section class="surface">
        <div class="surface-head">
          <div class="row">
            <span class="mono">Thư mục: <strong>${esc(currentDir)}</strong></span>
            ${currentDir !== '/' ? '<button type="button" class="btn btn-sm" id="fm-up-btn">Lên 1 cấp</button>' : ''}
          </div>
          <span class="meta-line"><span>${visible.length} mục</span></span>
        </div>
        <div class="table-wrap">
          <table class="data-table">
            <thead>
              <tr><th>Tên</th><th>Loại</th><th>Quyền</th><th class="right">Kích thước</th><th class="right">Thao tác</th></tr>
            </thead>
            <tbody>
              ${
                visible.length
                  ? visible.map((n) => `
                    <tr>
                      <td>
                        ${n.type === 'directory'
                          ? `<button type="button" class="btn btn-sm" data-open-dir="${esc(n.path)}">${esc(n.name)}/</button>`
                          : `<span class="mono">${esc(n.name)}</span>`}
                      </td>
                      <td>${n.type === 'directory' ? 'Thư mục' : 'Tệp'}</td>
                      <td class="mono">${esc(n.permissions || '0644')}</td>
                      <td class="right mono">${n.type === 'directory' ? '—' : esc(fmtBytes(n.sizeBytes))}</td>
                      <td class="right">
                        <div class="row" style="justify-content:flex-end">
                          ${n.type !== 'directory' ? `<button type="button" class="btn btn-sm" data-edit-file="${esc(n.id)}">Sửa</button>` : ''}
                          <button type="button" class="btn btn-sm" data-rename-file="${esc(n.id)}" data-name="${esc(n.name)}">Đổi tên</button>
                          ${/\.(zip|tar\.gz|tgz|tar\.zst)$/i.test(n.name) ? `<button type="button" class="btn btn-sm" data-extract-file="${esc(n.id)}">Giải nén</button>` : ''}
                          <button type="button" class="btn btn-sm btn-danger" data-del-file="${esc(n.id)}" data-name="${esc(n.name)}">Xóa</button>
                        </div>
                      </td>
                    </tr>`).join('')
                  : '<tr><td colspan="5" class="empty-state">Thư mục trống.</td></tr>'
              }
            </tbody>
          </table>
        </div>
      </section>
    `;

    document.getElementById('fm-app-select')?.addEventListener('change', (e) => {
      selectedAppId = e.target.value;
      currentDir = '/';
      loadAndDraw();
    });
    document.getElementById('fm-up-btn')?.addEventListener('click', () => {
      const parts = currentDir.split('/').filter(Boolean);
      parts.pop();
      currentDir = parts.length ? `/${parts.join('/')}` : '/';
      loadAndDraw();
    });
    container.querySelectorAll('[data-open-dir]').forEach((btn) => {
      btn.addEventListener('click', () => { currentDir = btn.dataset.openDir; loadAndDraw(); });
    });

    document.getElementById('fm-new-file')?.addEventListener('click', () => openCreateNodeModal('file'));
    document.getElementById('fm-new-dir')?.addEventListener('click', () => openCreateNodeModal('directory'));

    document.getElementById('fm-compress-btn')?.addEventListener('click', () => {
      openModal('Nén thư mục hiện tại (.tar.gz)', `
        <form id="fm-comp-form">
          <div class="field"><label>Thư mục nguồn</label><input class="input mono" name="sourceDir" value="${esc(currentDir)}" required /></div>
          <div class="field"><label>Tên file nén</label><input class="input mono" name="archiveName" value="archive.tar.gz" required /></div>
          <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="fm-comp-sub">Nén ngay</button></div>
        </form>
      `, {
        onMount(body) {
          body.querySelector('#fm-comp-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            await withBusy(body.querySelector('#fm-comp-sub'), async () => {
              const fd = new FormData(e.target);
              try {
                await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/compress`, {
                  sourceDir: fd.get('sourceDir'),
                  archiveName: fd.get('archiveName'),
                });
                closeModal();
                toast('Đã tạo lưu trữ thành công.');
                await loadAndDraw();
              } catch (err) { toast(err.message, 'err'); }
            });
          });
        },
      });
    });

    document.getElementById('fm-remote-dl-btn')?.addEventListener('click', () => {
      openModal('Tải tệp từ URL công khai (SSRF Guard)', `
        <form id="fm-rdl-form">
          <div class="field"><label>URL (HTTP/HTTPS công khai)</label><input class="input mono" name="url" placeholder="https://example.com/release.tar.gz" required /></div>
          <div class="field"><label>Tên tệp lưu lại</label><input class="input mono" name="fileName" placeholder="release.tar.gz" required /></div>
          <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="fm-rdl-sub">Tải về máy chủ</button></div>
        </form>
      `, {
        onMount(body) {
          body.querySelector('#fm-rdl-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            await withBusy(body.querySelector('#fm-rdl-sub'), async () => {
              const fd = new FormData(e.target);
              try {
                await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/remote-download`, {
                  url: fd.get('url'),
                  parentDir: currentDir,
                  fileName: fd.get('fileName'),
                });
                closeModal();
                toast('Đã tải tệp về thư mục an toàn.');
                await loadAndDraw();
              } catch (err) { toast(err.message, 'err'); }
            });
          });
        },
      });
    });

    document.getElementById('fm-chmod-btn')?.addEventListener('click', () => {
      const btn = document.getElementById('fm-chmod-btn');
      confirmAction('Chuẩn hóa quyền thư mục', 'Áp dụng quyền tiêu chuẩn (0755 cho thư mục, 0644 cho tệp) cho toàn bộ website này?', () => withBusy(btn, async () => {
        try {
          await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/batch-chmod`, {});
          toast('Đã chuẩn hóa quyền tệp/thư mục.');
          await loadAndDraw();
        } catch (err) { toast(err.message, 'err'); }
      }), false);
    });

    document.getElementById('fm-upload-input')?.addEventListener('change', async (e) => {
      const file = e.target.files?.[0];
      if (!file) return;
      try {
        const buf = await file.arrayBuffer();
        const url = `/api/files/${encodeURIComponent(selectedAppId)}/upload-raw?parentDir=${encodeURIComponent(currentDir)}&fileName=${encodeURIComponent(file.name)}`;
        await api('POST', url, buf, { rawBody: true, contentType: 'application/octet-stream' });
        toast(`Đã tải lên ${file.name} (${fmtBytes(file.size)})`);
        await loadAndDraw();
      } catch (err) {
        toast(err.message, 'err');
      }
    });

    container.querySelectorAll('[data-edit-file]').forEach((btn) => {
      btn.addEventListener('click', () => {
        const node = nodes.find((x) => x.id === btn.dataset.editFile);
        if (!node) return;
        openModal(`Chỉnh sửa: ${node.path}`, `
          <form id="fm-edit-form">
            <div class="field">
              <label>Nội dung tệp</label>
              <textarea class="textarea" name="content" style="min-height:320px">${esc(node.content || '')}</textarea>
            </div>
            <div class="row" style="justify-content:flex-end">
              <button type="submit" class="btn btn-primary" id="fm-edit-sub">Lưu thay đổi</button>
            </div>
          </form>
        `, {
          large: true,
          onMount(body) {
            body.querySelector('#fm-edit-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              await withBusy(body.querySelector('#fm-edit-sub'), async () => {
                const content = body.querySelector('textarea[name="content"]').value;
                try {
                  await api('PUT', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(node.id)}`, { content });
                  closeModal();
                  toast(`Đã lưu ${node.name}`);
                  await loadAndDraw();
                } catch (err) {
                  toast(err.message, 'err');
                }
              });
            });
          },
        });
      });
    });

    container.querySelectorAll('[data-rename-file]').forEach((btn) => {
      btn.addEventListener('click', () => {
        openModal(`Đổi tên: ${btn.dataset.name}`, `
          <form id="fm-ren-form">
            <div class="field"><label>Tên mới</label><input class="input mono" name="newName" value="${esc(btn.dataset.name)}" required /></div>
            <div class="row" style="justify-content:flex-end"><button type="submit" class="btn btn-primary" id="fm-ren-sub">Đổi tên</button></div>
          </form>
        `, {
          onMount(body) {
            body.querySelector('#fm-ren-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              await withBusy(body.querySelector('#fm-ren-sub'), async () => {
                const newName = new FormData(e.target).get('newName');
                try {
                  await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(btn.dataset.renameFile)}/rename`, { newName });
                  closeModal();
                  toast('Đã đổi tên thành công.');
                  await loadAndDraw();
                } catch (err) { toast(err.message, 'err'); }
              });
            });
          },
        });
      });
    });

    container.querySelectorAll('[data-extract-file]').forEach((btn) => {
      btn.addEventListener('click', () => withBusy(btn, async () => {
        try {
          await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(btn.dataset.extractFile)}/extract`, { targetDir: currentDir });
          toast('Đã giải nén lưu trữ an toàn.');
          await loadAndDraw();
        } catch (err) {
          toast(err.message, 'err');
        }
      }));
    });

    container.querySelectorAll('[data-del-file]').forEach((btn) => {
      btn.addEventListener('click', () => {
        confirmAction('Xóa tệp / thư mục', `Xác nhận xóa vĩnh viễn ${btn.dataset.name}?`, () => withBusy(btn, async () => {
          try {
            await api('DELETE', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(btn.dataset.delFile)}`);
            toast(`Đã xóa ${btn.dataset.name}`);
            await loadAndDraw();
          } catch (err) {
            toast(err.message, 'err');
          }
        }));
      });
    });
  }

  function openCreateNodeModal(nodeType) {
    openModal(nodeType === 'directory' ? 'Tạo thư mục mới' : 'Tạo tệp mới', `
      <form id="fm-create-form">
        <div class="field">
          <label>Tên ${nodeType === 'directory' ? 'thư mục' : 'tệp'}</label>
          <input class="input mono" name="name" placeholder="${nodeType === 'directory' ? 'assets' : 'index.html'}" required />
        </div>
        ${nodeType === 'file' ? '<div class="field"><label>Nội dung ban đầu</label><textarea class="textarea" name="content"></textarea></div>' : ''}
        <div class="row" style="justify-content:flex-end">
          <button type="submit" class="btn btn-primary" id="fm-create-sub">Tạo mới</button>
        </div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#fm-create-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#fm-create-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}`, {
                name: String(fd.get('name') || '').trim(),
                parentDir: currentDir,
                type: nodeType,
                content: nodeType === 'file' ? String(fd.get('content') || '') : null,
              });
              closeModal();
              toast('Đã tạo thành công.');
              await loadAndDraw();
            } catch (err) {
              toast(err.message, 'err');
            }
          });
        });
      },
    });
  }

  await loadAndDraw();
}

// ============================================================================
// 4. SQL DATABASES & PURE-RUST FERROUSADMIN
// ============================================================================
export async function renderDatabases(container) {
  const dbs = await api('GET', '/api/databases');

  container.innerHTML = `
    <div class="page-head">
      <div>
        <h1 class="page-title">Cơ sở dữ liệu SQL (Pure-Rust Engine)</h1>
        <p class="page-desc">Quản lý SQLite 3 (WAL thuần Rust), MariaDB 11.4 và PostgreSQL 17, truyền mật khẩu an toàn qua stdin, xuất/nhập SQL streaming.</p>
      </div>
      <div class="row">
        <a class="btn" href="/phpadmin/" target="_blank" rel="noopener">Mở FerrousAdmin Studio (/phpadmin/)</a>
        <button type="button" class="btn btn-primary" id="db-create-btn">Tạo Database mới</button>
      </div>
    </div>

    ${capBannerHtml(['mariadb', 'postgresql'])}

    <section class="surface">
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr><th>Tên Database</th><th>Engine & Trạng thái</th><th>Tài khoản</th><th class="right">Kết nối / Bảng / Dung lượng</th><th class="right">Thao tác</th></tr>
          </thead>
          <tbody>
            ${
              Array.isArray(dbs) && dbs.length
                ? dbs.map((d) => {
                  const tblCount = Array.isArray(d.tables) ? d.tables.length : 0;
                  const connStr = `${d.activeConnections ?? 0}/${d.maxConnections ?? 0}`;
                  return `
                  <tr>
                    <td><strong class="mono">${esc(d.name)}</strong></td>
                    <td>
                      <div>${esc(d.engine)}</div>
                      <div class="meta-line"><span class="${statusClass(d.status)}">${esc(d.status || 'online')}</span></div>
                    </td>
                    <td class="mono">${esc(d.username)}</td>
                    <td class="right mono">${esc(connStr)} conn · ${esc(tblCount)} bảng · ${Number(d.sizeMb || 0).toFixed(2)} MB</td>
                    <td class="right">
                      <div class="row" style="justify-content:flex-end">
                        <button type="button" class="btn btn-sm" data-db-sql="${esc(d.id)}" data-name="${esc(d.name)}">Chạy SQL</button>
                        <button type="button" class="btn btn-sm" data-db-pw="${esc(d.id)}" data-name="${esc(d.name)}">Đổi MK</button>
                        <button type="button" class="btn btn-sm" data-db-export="${esc(d.id)}" data-name="${esc(d.name)}">Xuất .sql</button>
                        <label class="btn btn-sm" style="cursor:pointer">
                          Nhập .sql
                          <input type="file" accept=".sql,.txt" hidden data-db-import="${esc(d.id)}" />
                        </label>
                        <button type="button" class="btn btn-sm btn-danger" data-db-del="${esc(d.id)}" data-name="${esc(d.name)}">Xóa</button>
                      </div>
                    </td>
                  </tr>`;
                }).join('')
                : '<tr><td colspan="5" class="empty-state">Chưa có cơ sở dữ liệu nào. Bạn có thể tạo ngay SQLite 3 (WAL · Pure Rust) mà không cần cài đặt thêm dịch vụ.</td></tr>'
            }
          </tbody>
        </table>
      </div>
    </section>
  `;

  document.getElementById('db-create-btn')?.addEventListener('click', () => {
    openModal('Tạo cơ sở dữ liệu SQL mới', `
      <form id="db-create-form">
        <div class="form-grid">
          <div class="field">
            <label>Tên Database</label>
            <input class="input mono" name="name" placeholder="app_prod_db" required />
          </div>
          <div class="field">
            <label>Hệ quản trị SQL</label>
            <select class="select" name="engine">
              <option value="SQLite 3 (WAL · Pure Rust)">SQLite 3 (WAL · Pure Rust)</option>
              <option value="MariaDB 11.4">MariaDB 11.4</option>
              <option value="PostgreSQL 17">PostgreSQL 17</option>
            </select>
          </div>
          <div class="field">
            <label>Tài khoản DB</label>
            <input class="input mono" name="username" placeholder="app_usr" />
          </div>
          <div class="field">
            <label>Mật khẩu (để trống = tự sinh)</label>
            <input class="input mono" name="password" type="password" />
          </div>
        </div>
        <div class="row" style="justify-content:flex-end;margin-top:12px">
          <button type="submit" class="btn btn-primary" id="db-create-sub">Tạo Database</button>
        </div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#db-create-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
          await withBusy(body.querySelector('#db-create-sub'), async () => {
            const fd = new FormData(e.target);
            try {
              await api('POST', '/api/databases', {
                name: String(fd.get('name') || '').trim(),
                engine: String(fd.get('engine') || 'SQLite 3 (WAL · Pure Rust)'),
                username: String(fd.get('username') || '').trim(),
                password: String(fd.get('password') || '').trim() || null,
                collation: 'UTF-8',
                linkedAppId: '',
              });
              closeModal();
              toast('Đã tạo database mới.');
              await renderDatabases(container);
            } catch (err) {
              toast(err.message, 'err');
            }
          });
        });
      },
    });
  });

  container.querySelectorAll('[data-db-sql]').forEach((btn) => {
    btn.addEventListener('click', () => {
      openModal(`SQL Console — ${btn.dataset.name}`, `
        <form id="sql-run-form">
          <div class="field">
            <label>Câu lệnh SQL</label>
            <textarea class="textarea" name="query" placeholder="SELECT sqlite_version();"></textarea>
          </div>
          <div class="row" style="justify-content:flex-end;margin-bottom:12px">
            <button type="submit" class="btn btn-primary" id="sql-run-sub">Thực thi SQL</button>
          </div>
        </form>
        <div id="sql-result-box"></div>
      `, {
        large: true,
        onMount(body) {
          body.querySelector('#sql-run-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            await withBusy(body.querySelector('#sql-run-sub'), async () => {
              const query = body.querySelector('textarea[name="query"]').value;
              const out = body.querySelector('#sql-result-box');
              out.innerHTML = '<div class="empty-state">Đang thực thi…</div>';
              try {
                const res = await api('POST', `/api/databases/${encodeURIComponent(btn.dataset.dbSql)}/query`, { query });
                const cols = res.columns || [];
                const rows = res.rows || [];
                out.innerHTML = `
                  <div class="meta-line" style="margin-bottom:8px">
                    <span class="${res.success ? 'st-ok' : 'st-err'}">${esc(res.commandTag || 'OK')}</span>
                    <span>·</span>
                    <span class="mono">${Number(res.executionTimeMs || 0).toFixed(2)} ms</span>
                    <span>·</span>
                    <span class="mono">${rows.length} dòng</span>
                  </div>
                  ${cols.length ? `
                    <div class="table-wrap">
                      <table class="data-table">
                        <thead><tr>${cols.map((c) => `<th class="mono">${esc(c)}</th>`).join('')}</tr></thead>
                        <tbody>${rows.map((r) => `<tr>${cols.map((c) => `<td class="mono">${esc(r[c] ?? 'NULL')}</td>`).join('')}</tr>`).join('')}</tbody>
                      </table>
                    </div>
                  ` : '<div class="empty-state">Thực thi thành công (không có tập kết quả trả về).</div>'}
                `;
              } catch (err) {
                out.innerHTML = `<div class="alert-err">${esc(err.message)}</div>`;
              }
            });
          });
        },
      });
    });
  });

  container.querySelectorAll('[data-db-export]').forEach((btn) => {
    btn.addEventListener('click', () => withBusy(btn, async () => {
      try {
        const blob = await api('GET', `/api/databases/${encodeURIComponent(btn.dataset.dbExport)}/export?download=1`, undefined, { responseType: 'blob' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${btn.dataset.name}.sql`;
        a.click();
        URL.revokeObjectURL(url);
        toast(`Đã tải bản sao lưu ${btn.dataset.name}.sql`);
      } catch (err) {
        toast(err.message, 'err');
      }
    }));
  });

  container.querySelectorAll('[data-db-import]').forEach((input) => {
    input.addEventListener('change', async (e) => {
      const file = e.target.files?.[0];
      if (!file) return;
      try {
        const text = await file.text();
        await api('POST', `/api/databases/${encodeURIComponent(input.dataset.dbImport)}/import`, text, { rawBody: true, contentType: 'application/sql' });
        toast(`Đã nhập tệp ${file.name} thành công.`);
        await renderDatabases(container);
      } catch (err) {
        toast(err.message, 'err');
      }
    });
  });

  container.querySelectorAll('[data-db-pw]').forEach((btn) => {
    btn.addEventListener('click', () => {
      openModal(`Đổi mật khẩu Database — ${btn.dataset.name}`, `
        <form id="db-pw-form">
          <div class="field">
            <label>Mật khẩu mới (tối thiểu 8 ký tự)</label>
            <input class="input mono" name="password" type="password" minlength="8" required />
          </div>
          <div class="row" style="justify-content:flex-end">
            <button type="submit" class="btn btn-primary" id="db-pw-sub">Cập nhật mật khẩu</button>
          </div>
        </form>
      `, {
        onMount(body) {
          body.querySelector('#db-pw-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
            await withBusy(body.querySelector('#db-pw-sub'), async () => {
              const password = String(new FormData(e.target).get('password') || '');
              try {
                await api('PUT', `/api/databases/${encodeURIComponent(btn.dataset.dbPw)}/password`, { password });
                closeModal();
                toast(`Đã cập nhật mật khẩu cho ${btn.dataset.name}.`);
              } catch (err) {
                toast(err.message, 'err');
              }
            });
          });
        },
      });
    });
  });

  container.querySelectorAll('[data-db-del]').forEach((btn) => {
    btn.addEventListener('click', () => {
      confirmAction('Xóa Database', `Xác nhận xóa vĩnh viễn database ${btn.dataset.name}? Dữ liệu sẽ không thể khôi phục.`, () => withBusy(btn, async () => {
        try {
          await api('DELETE', `/api/databases/${encodeURIComponent(btn.dataset.dbDel)}`);
          toast(`Đã xóa database ${btn.dataset.name}`);
          await renderDatabases(container);
        } catch (err) {
          toast(err.message, 'err');
        }
      }));
    });
  });
}
