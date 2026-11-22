// FerrousPanel Pure-Rust UI — Core Views (Overview, Websites, Files, SQL Databases)
import {
  api, esc, fmtBytes, fmtTime, fmtUptime, statusClass,
  toast, openModal, closeModal, confirmAction, openJobDrawer, navigate, state,
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
// 1. OVERVIEW / TELEMETRY
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
        <div class="kpi-sub">Web Engine: <strong class="mono">${esc(activeEngine)}</strong> · Active conn: <span class="mono">${esc(tel.activeConnections || 0)}</span></div>
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
  `;

  document.getElementById('ov-new-app')?.addEventListener('click', () => navigate('apps'));
  document.getElementById('ov-open-db')?.addEventListener('click', () => navigate('databases'));
  document.getElementById('ov-goto-ws')?.addEventListener('click', () => navigate('webserver'));
  container.querySelectorAll('[data-view-job]').forEach((btn) => {
    btn.addEventListener('click', () => openJobDrawer(btn.dataset.viewJob));
  });

  state.refreshTimer = setInterval(() => {
    if (state.route === 'overview') renderOverview(container);
  }, 10000);
}

// ============================================================================
// 2. WEBSITES & APPS + 1-CLICK TEMPLATES
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
                          <div class="meta-line"><span>${esc(a.webServerEngine || 'nginx')}</span><span>·</span><span>SSL: ${a.sslEnabled ? 'Bật' : 'Tắt'}</span></div>
                        </td>
                        <td><span class="${statusClass(a.status)}">${esc(a.status)}</span></td>
                        <td class="mono">${esc(a.internalPort || '—')}</td>
                        <td class="right">
                          <div class="row" style="justify-content:flex-end">
                            <button type="button" class="btn btn-sm" data-app-act="restart" data-id="${esc(a.id)}">Restart</button>
                            <button type="button" class="btn btn-sm" data-app-job="rebuild" data-id="${esc(a.id)}">Rebuild</button>
                            <button type="button" class="btn btn-sm" data-app-job="renew-ssl" data-id="${esc(a.id)}">SSL</button>
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
                    <span>${esc(t.runtime || '')}</span>
                  </div>
                  <button type="button" class="btn btn-primary btn-sm" data-oc-deploy="${esc(t.id)}" data-oc-name="${esc(t.name)}">Triển khai ngay</button>
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

    container.querySelectorAll('[data-app-act]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          await api('POST', `/api/apps/${encodeURIComponent(btn.dataset.id)}/action`, { action: btn.dataset.appAct });
          toast('Đã gửi lệnh khởi động lại dịch vụ.');
          await renderApps(container);
        } catch (err) {
          toast(err.message, 'err');
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('[data-app-job]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          const job = await api('POST', `/api/jobs/apps/${encodeURIComponent(btn.dataset.id)}/${btn.dataset.appJob}`, {});
          toast(`Đã xếp hàng tác vụ ${job.title}`);
          openJobDrawer(job.id);
        } catch (err) {
          toast(err.message, 'err');
        } finally {
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('[data-app-probe]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
          const res = await api('POST', `/api/apps/${encodeURIComponent(btn.dataset.appProbe)}/test-http`, {});
          toast(`HTTP ${res.statusCode || 200} · ${res.latencyMs || 0} ms · ${res.protocol || 'HTTP/1.1'}`);
        } catch (err) {
          toast(err.message, 'err');
        } finally {
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('[data-app-del]').forEach((btn) => {
      btn.addEventListener('click', () => {
        confirmAction('Xóa Website', `Xác nhận xóa website ${btn.dataset.domain}?`, async () => {
          btn.disabled = true;
          try {
            await api('DELETE', `/api/apps/${encodeURIComponent(btn.dataset.appDel)}`);
            toast(`Đã xóa ${btn.dataset.domain}`);
            await renderApps(container);
          } catch (err) {
            toast(err.message, 'err');
            btn.disabled = false;
          }
        });
      });
    });

    container.querySelectorAll('[data-oc-deploy]').forEach((btn) => {
      btn.addEventListener('click', () => {
        openModal(`Triển khai 1-Click: ${btn.dataset.ocName}`, `
          <form id="oc-form">
            <div class="field">
              <label>Tên miền triển khai</label>
              <input class="input" name="domain" placeholder="blog.example.com" required />
            </div>
            <div class="field">
              <label>Tiêu đề trang</label>
              <input class="input" name="siteTitle" value="${esc(btn.dataset.ocName)}" />
            </div>
            <div class="field">
              <label>Email quản trị (dùng cho SSL / Admin)</label>
              <input class="input" name="adminEmail" type="email" placeholder="admin@example.com" />
            </div>
            <div class="row" style="justify-content:flex-end;margin-top:16px">
              <button type="submit" class="btn btn-primary">Bắt đầu triển khai</button>
            </div>
          </form>
        `, {
          onMount(body) {
            body.querySelector('#oc-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
              const fd = new FormData(e.target);
              try {
                await api('POST', '/api/oneclick/deploy', {
                  templateId: btn.dataset.ocDeploy,
                  domain: fd.get('domain'),
                  siteTitle: fd.get('siteTitle') || '',
                  adminEmail: fd.get('adminEmail') || '',
                  enableSsl: false,
                  autoCreateDb: true,
                });
                closeModal();
                toast('Đã triển khai ứng dụng 1-Click.');
                await renderApps(container);
              } catch (err) {
                toast(err.message, 'err');
              }
            });
          },
        });
      });
    });
  }

  draw();
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
      </div>
      <div class="row" style="justify-content:flex-end">
        <button type="submit" class="btn btn-primary">Tạo & Triển khai</button>
      </div>
    </form>
  `, {
    onMount(body) {
      body.querySelector('#deploy-app-form')?.addEventListener('submit', async (e) => {
        e.preventDefault();
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
            http3Quic: true,
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
    },
  });
}

// ============================================================================
// 3. SAFEPATH FILE MANAGER
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
          <p class="page-desc">Chỉnh sửa trực tiếp mã nguồn, tải lên tệp nhị phân raw tới 64 MiB và giải nén lưu trữ an toàn.</p>
        </div>
        <div class="row">
          <select class="select" id="fm-app-select" style="width:auto;min-width:220px">
            ${apps.map((a) => `<option value="${esc(a.id)}" ${a.id === selectedAppId ? 'selected' : ''}>${esc(a.domain)}</option>`).join('')}
          </select>
          <button type="button" class="btn" id="fm-new-file">Tạo tệp</button>
          <button type="button" class="btn" id="fm-new-dir">Tạo thư mục</button>
          <label class="btn btn-primary" style="cursor:pointer">
            Tải tệp lên
            <input type="file" id="fm-upload-input" hidden />
          </label>
        </div>
      </div>

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
                          ${n.name.endsWith('.zip') ? `<button type="button" class="btn btn-sm" data-extract-file="${esc(n.id)}">Giải nén</button>` : ''}
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
              <button type="submit" class="btn btn-primary">Lưu thay đổi</button>
            </div>
          </form>
        `, {
          large: true,
          onMount(body) {
            body.querySelector('#fm-edit-form')?.addEventListener('submit', async (e) => {
              e.preventDefault();
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
          },
        });
      });
    });

    container.querySelectorAll('[data-extract-file]').forEach((btn) => {
      btn.addEventListener('click', async () => {
        try {
          await api('POST', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(btn.dataset.extractFile)}/extract`, { targetDir: currentDir });
          toast('Đã giải nén lưu trữ.');
          await loadAndDraw();
        } catch (err) {
          toast(err.message, 'err');
        }
      });
    });

    container.querySelectorAll('[data-del-file]').forEach((btn) => {
      btn.addEventListener('click', () => {
        confirmAction('Xóa tệp / thư mục', `Xác nhận xóa ${btn.dataset.name}?`, async () => {
          try {
            await api('DELETE', `/api/files/${encodeURIComponent(selectedAppId)}/${encodeURIComponent(btn.dataset.delFile)}`);
            toast(`Đã xóa ${btn.dataset.name}`);
            await loadAndDraw();
          } catch (err) {
            toast(err.message, 'err');
          }
        });
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
          <button type="submit" class="btn btn-primary">Tạo mới</button>
        </div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#fm-create-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
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
        <p class="page-desc">Quản lý SQLite 3 (WAL thuần Rust), MariaDB 11.4 và PostgreSQL 17, xuất/nhập SQL streaming tới 64 MiB.</p>
      </div>
      <div class="row">
        <a class="btn" href="/phpadmin/" target="_blank" rel="noopener">Mở FerrousAdmin Studio (/phpadmin/)</a>
        <button type="button" class="btn btn-primary" id="db-create-btn">Tạo Database mới</button>
      </div>
    </div>

    <section class="surface">
      <div class="table-wrap">
        <table class="data-table">
          <thead>
            <tr><th>Tên Database</th><th>Engine</th><th>Tài khoản</th><th class="right">Bảng / Kích thước</th><th class="right">Thao tác</th></tr>
          </thead>
          <tbody>
            ${
              Array.isArray(dbs) && dbs.length
                ? dbs.map((d) => `
                  <tr>
                    <td><strong class="mono">${esc(d.name)}</strong></td>
                    <td>${esc(d.engine)}</td>
                    <td class="mono">${esc(d.username)}</td>
                    <td class="right mono">${esc(d.tablesCount ?? 0)} bảng · ${Number(d.sizeMb || 0).toFixed(2)} MB</td>
                    <td class="right">
                      <div class="row" style="justify-content:flex-end">
                        <button type="button" class="btn btn-sm" data-db-sql="${esc(d.id)}" data-name="${esc(d.name)}">Chạy SQL</button>
                        <button type="button" class="btn btn-sm" data-db-export="${esc(d.id)}" data-name="${esc(d.name)}">Xuất .sql</button>
                        <label class="btn btn-sm" style="cursor:pointer">
                          Nhập .sql
                          <input type="file" accept=".sql,.txt" hidden data-db-import="${esc(d.id)}" />
                        </label>
                        <button type="button" class="btn btn-sm btn-danger" data-db-del="${esc(d.id)}" data-name="${esc(d.name)}">Xóa</button>
                      </div>
                    </td>
                  </tr>`).join('')
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
          <button type="submit" class="btn btn-primary">Tạo Database</button>
        </div>
      </form>
    `, {
      onMount(body) {
        body.querySelector('#db-create-form')?.addEventListener('submit', async (e) => {
          e.preventDefault();
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
            <button type="submit" class="btn btn-primary">Thực thi SQL</button>
          </div>
        </form>
        <div id="sql-result-box"></div>
      `, {
        large: true,
        onMount(body) {
          body.querySelector('#sql-run-form')?.addEventListener('submit', async (e) => {
            e.preventDefault();
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
        },
      });
    });
  });

  container.querySelectorAll('[data-db-export]').forEach((btn) => {
    btn.addEventListener('click', async () => {
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
    });
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

  container.querySelectorAll('[data-db-del]').forEach((btn) => {
    btn.addEventListener('click', () => {
      confirmAction('Xóa Database', `Xác nhận xóa database ${btn.dataset.name}?`, async () => {
        try {
          await api('DELETE', `/api/databases/${encodeURIComponent(btn.dataset.dbDel)}`);
          toast(`Đã xóa database ${btn.dataset.name}`);
          await renderDatabases(container);
        } catch (err) {
          toast(err.message, 'err');
        }
      });
    });
  });
}
