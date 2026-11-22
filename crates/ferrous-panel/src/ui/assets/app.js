// FerrousPanel Pure-Rust Native Frontend Core (Zero Node.js)
import { renderOverview, renderApps, renderFiles, renderDatabases } from './views-core.js';
import { renderWebserver, renderPhpSoftware, renderCpanel, renderMail } from './views-infra.js';
import { renderSecurity, renderOptimizer, renderSsh, renderSettingsJobs } from './views-security.js';

const TOKEN_KEY = 'fp_session_token';
const THEME_KEY = 'fp_theme';

export const state = {
  boot: readBootConfig(),
  token: sessionStorage.getItem(TOKEN_KEY) || localStorage.getItem(TOKEN_KEY) || '',
  user: null,
  route: 'overview',
  mobileNavOpen: false,
  activeJobPoll: null,
  telemetryHistory: [],
  refreshTimer: null,
};

function readBootConfig() {
  try {
    const el = document.getElementById('ferrous-boot');
    if (el && el.textContent) return JSON.parse(el.textContent);
  } catch (_) {
    // ignore malformed boot block
  }
  return { version: '2.5.0', panelPort: 8443, securityEntrance: '/panel', customDomain: null, uiEngine: 'pure-rust-native' };
}

export function esc(val) {
  return String(val ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

export function fmtBytes(bytes) {
  const n = Number(bytes) || 0;
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function fmtUptime(secs) {
  const s = Math.max(0, Math.floor(Number(secs) || 0));
  const d = Math.floor(s / 86400);
  const h = Math.floor((s % 86400) / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m ${s % 60}s`;
}

export function fmtTime(iso) {
  if (!iso) return '—';
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleString('vi-VN', { hour12: false });
}

export function statusClass(status) {
  const s = String(status || '').toLowerCase();
  if (['running', 'active', 'online', 'completed', 'ok', 'healthy', 'installed', 'enabled', 'pass'].includes(s)) return 'st-ok';
  if (['queued', 'pending', 'building', 'warning', 'warn', 'elevated', 'degraded'].includes(s)) return 'st-warn';
  if (['failed', 'error', 'stopped', 'critical', 'under_attack', 'offline', 'missing'].includes(s)) return 'st-err';
  return 'st-muted';
}

export function toast(message, kind = 'ok') {
  const root = document.getElementById('toast-root');
  if (!root) return;
  const el = document.createElement('div');
  el.className = `toast ${kind === 'err' ? 'err' : 'ok'}`;
  el.innerHTML = `<span>${esc(message)}</span>`;
  root.appendChild(el);
  setTimeout(() => {
    el.remove();
  }, 4200);
}

export function closeModal() {
  const root = document.getElementById('modal-root');
  if (root) root.innerHTML = '';
}

export function openModal(title, bodyHtml, { large = false, onMount } = {}) {
  const root = document.getElementById('modal-root');
  if (!root) return;
  root.innerHTML = `
    <div class="backdrop" id="fp-modal-backdrop">
      <div class="modal ${large ? 'modal-lg' : ''}" role="dialog" aria-modal="true">
        <div class="modal-head">
          <h3 class="modal-title">${esc(title)}</h3>
          <button type="button" class="btn btn-sm" id="fp-modal-close">Đóng</button>
        </div>
        <div id="fp-modal-body">${bodyHtml}</div>
      </div>
    </div>
  `;
  const backdrop = document.getElementById('fp-modal-backdrop');
  const closeBtn = document.getElementById('fp-modal-close');
  closeBtn?.addEventListener('click', closeModal);
  backdrop?.addEventListener('click', (e) => {
    if (e.target === backdrop) closeModal();
  });
  if (typeof onMount === 'function') onMount(document.getElementById('fp-modal-body'));
}

export function confirmAction(title, message, onConfirm, danger = true) {
  openModal(title, `
    <p class="page-desc" style="margin-bottom:18px;color:var(--text-secondary)">${esc(message)}</p>
    <div class="row" style="justify-content:flex-end">
      <button type="button" class="btn" id="fp-confirm-cancel">Hủy</button>
      <button type="button" class="btn ${danger ? 'btn-danger' : 'btn-primary'}" id="fp-confirm-ok">Xác nhận</button>
    </div>
  `, {
    onMount(body) {
      body.querySelector('#fp-confirm-cancel')?.addEventListener('click', closeModal);
      body.querySelector('#fp-confirm-ok')?.addEventListener('click', async () => {
        const okBtn = body.querySelector('#fp-confirm-ok');
        if (okBtn) okBtn.disabled = true;
        closeModal();
        await onConfirm();
      });
    },
  });
}

export function closeJobDrawer() {
  state.activeJobPoll = null;
  const root = document.getElementById('drawer-root');
  if (root) root.innerHTML = '';
}

export async function openJobDrawer(jobId) {
  const root = document.getElementById('drawer-root');
  if (!root) return;
  const pollToken = Symbol('job-poll');
  state.activeJobPoll = pollToken;

  root.innerHTML = `
    <div class="drawer-backdrop" id="fp-drawer-backdrop">
      <div class="drawer" role="dialog" aria-modal="true">
        <div class="row-between">
          <div>
            <h3 class="modal-title" id="job-drawer-title">Tác vụ nền</h3>
            <div class="meta-line mono" id="job-drawer-meta">${esc(jobId)}</div>
          </div>
          <button type="button" class="btn btn-sm" id="job-drawer-close">Đóng</button>
        </div>
        <div class="bar-track"><div class="bar-fill" id="job-drawer-bar" style="width:10%"></div></div>
        <pre class="terminal-box" id="job-drawer-log" style="flex:1;max-height:none">Đang kết nối luồng log thời gian thực…</pre>
      </div>
    </div>
  `;
  document.getElementById('job-drawer-close')?.addEventListener('click', closeJobDrawer);
  document.getElementById('fp-drawer-backdrop')?.addEventListener('click', (e) => {
    if (e.target.id === 'fp-drawer-backdrop') closeJobDrawer();
  });

  let offset = 0;
  const logEl = document.getElementById('job-drawer-log');
  if (logEl) logEl.textContent = '';

  while (state.activeJobPoll === pollToken) {
    try {
      const res = await api('GET', `/api/jobs/${encodeURIComponent(jobId)}/poll?offset=${offset}&waitMs=8000`);
      if (state.activeJobPoll !== pollToken) break;
      const job = res.job || {};
      const titleEl = document.getElementById('job-drawer-title');
      const metaEl = document.getElementById('job-drawer-meta');
      const barEl = document.getElementById('job-drawer-bar');
      if (titleEl) titleEl.textContent = job.title || jobId;
      if (metaEl) metaEl.textContent = `${job.id || jobId} · ${job.status || 'running'} · ${job.progress ?? 0}%`;
      if (barEl) {
        barEl.style.width = `${Math.min(100, Math.max(5, Number(job.progress) || 0))}%`;
        barEl.className = `bar-fill ${job.status === 'failed' ? 'err' : job.status === 'completed' ? 'ok' : ''}`;
      }
      if (res.logChunk && logEl) {
        logEl.textContent += res.logChunk;
        logEl.scrollTop = logEl.scrollHeight;
      }
      offset = Number(res.nextOffset ?? offset);
      if (res.done) {
        if (job.error && logEl) {
          logEl.textContent += `\n[LỖI] ${job.error}\n`;
        }
        toast(job.status === 'completed' ? `Hoàn tất: ${job.title}` : `Thất bại: ${job.title}`, job.status === 'completed' ? 'ok' : 'err');
        break;
      }
    } catch (err) {
      if (logEl) logEl.textContent += `\n[Kết nối poll lỗi]: ${err.message}\n`;
      break;
    }
  }
}

export async function api(method, path, body = undefined, { rawBody = false, contentType = 'application/json', responseType = 'json' } = {}) {
  const headers = {};
  if (state.token) {
    headers['Authorization'] = `Bearer ${state.token}`;
  }
  const init = {
    method,
    credentials: 'same-origin',
    headers,
  };
  if (body !== undefined) {
    if (rawBody) {
      headers['Content-Type'] = contentType;
      init.body = body;
    } else {
      headers['Content-Type'] = 'application/json';
      init.body = JSON.stringify(body);
    }
  }

  const resp = await fetch(path, init);
  if (resp.status === 401 && path !== '/api/auth/login') {
    state.user = null;
    state.token = '';
    sessionStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(TOKEN_KEY);
    renderLogin('Phiên đăng nhập đã hết hạn. Vui lòng đăng nhập lại.');
    throw new Error('Unauthorized');
  }

  if (responseType === 'blob') {
    if (!resp.ok) {
      const txt = await resp.text();
      throw new Error(txt || `HTTP ${resp.status}`);
    }
    return resp.blob();
  }

  const text = await resp.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch (_) {
    data = text;
  }
  if (!resp.ok) {
    const msg = (data && (data.error || data.message)) || (typeof data === 'string' && data) || `HTTP ${resp.status}`;
    throw new Error(msg);
  }
  return data;
}

function applyTheme(theme) {
  const isDark = theme !== 'light';
  document.documentElement.classList.toggle('dark', isDark);
  localStorage.setItem(THEME_KEY, isDark ? 'dark' : 'light');
}

function toggleTheme() {
  const next = document.documentElement.classList.contains('dark') ? 'light' : 'dark';
  applyTheme(next);
  const btn = document.getElementById('theme-toggle-btn');
  if (btn) btn.textContent = next === 'dark' ? 'Giao diện sáng' : 'Giao diện tối';
}

const NAV_GROUPS = [
  {
    label: 'Điều hành cốt lõi',
    items: [
      { id: 'overview', label: 'Tổng quan hệ thống' },
      { id: 'apps', label: 'Website & Ứng dụng' },
      { id: 'files', label: 'Quản lý Tệp' },
      { id: 'databases', label: 'Cơ sở dữ liệu SQL' },
    ],
  },
  {
    label: 'Hạ tầng & Dịch vụ',
    items: [
      { id: 'webserver', label: 'Web Engine (Nginx/FX)' },
      { id: 'software', label: 'PHP-FPM & Phần mềm' },
      { id: 'cpanel', label: 'Cron, Backup & DNS' },
      { id: 'mail', label: 'Máy chủ Mail' },
    ],
  },
  {
    label: 'Bảo mật & Hệ thống',
    items: [
      { id: 'security', label: 'Bảo mật & Audit Log' },
      { id: 'optimizer', label: 'Tối ưu & Chẩn đoán' },
      { id: 'ssh', label: 'SSH & Terminal' },
      { id: 'settings', label: 'Tác vụ nền & Cài đặt' },
    ],
  },
];

const ROUTE_TITLES = Object.fromEntries(
  NAV_GROUPS.flatMap((g) => g.items.map((it) => [it.id, it.label])),
);

export function navigate(routeId) {
  if (!ROUTE_TITLES[routeId]) routeId = 'overview';
  state.route = routeId;
  state.mobileNavOpen = false;
  if (window.location.hash !== `#${routeId}`) {
    window.history.replaceState(null, '', `#${routeId}`);
  }
  updateShellChrome();
  renderCurrentView();
}

function updateShellChrome() {
  document.querySelectorAll('.nav-btn[data-route]').forEach((btn) => {
    btn.classList.toggle('active', btn.dataset.route === state.route);
  });
  const bc = document.getElementById('topbar-breadcrumb-curr');
  if (bc) bc.textContent = ROUTE_TITLES[state.route] || 'Tổng quan hệ thống';
  const sidebar = document.getElementById('fp-sidebar');
  if (sidebar) sidebar.classList.toggle('open', state.mobileNavOpen);
}

export async function renderCurrentView() {
  if (state.refreshTimer) {
    clearInterval(state.refreshTimer);
    state.refreshTimer = null;
  }
  const container = document.getElementById('view-container');
  if (!container) return;
  container.innerHTML = `<div class="surface"><div class="empty-state">Đang tải dữ liệu từ Rust daemon…</div></div>`;

  try {
    switch (state.route) {
      case 'overview':
        await renderOverview(container);
        break;
      case 'apps':
        await renderApps(container);
        break;
      case 'files':
        await renderFiles(container);
        break;
      case 'databases':
        await renderDatabases(container);
        break;
      case 'webserver':
        await renderWebserver(container);
        break;
      case 'software':
        await renderPhpSoftware(container);
        break;
      case 'cpanel':
        await renderCpanel(container);
        break;
      case 'mail':
        await renderMail(container);
        break;
      case 'security':
        await renderSecurity(container);
        break;
      case 'optimizer':
        await renderOptimizer(container);
        break;
      case 'ssh':
        await renderSsh(container);
        break;
      case 'settings':
        await renderSettingsJobs(container);
        break;
      default:
        await renderOverview(container);
    }
  } catch (err) {
    if (err.message !== 'Unauthorized') {
      container.innerHTML = `
        <div class="surface">
          <div class="alert-err">Không thể tải phân hệ: ${esc(err.message)}</div>
          <button type="button" class="btn" id="retry-view-btn">Thử tải lại</button>
        </div>
      `;
      document.getElementById('retry-view-btn')?.addEventListener('click', () => renderCurrentView());
    }
  }
}

export async function handleLogout() {
  try {
    await api('POST', '/api/auth/logout', { token: state.token || null });
  } catch (_) {
    // clear local session regardless
  }
  state.token = '';
  state.user = null;
  sessionStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(TOKEN_KEY);
  if (state.refreshTimer) {
    clearInterval(state.refreshTimer);
    state.refreshTimer = null;
  }
  toast('Đã đăng xuất khỏi FerrousPanel.');
  renderLogin();
}

function renderShell() {
  const root = document.getElementById('app');
  if (!root) return;
  const isDark = document.documentElement.classList.contains('dark');
  const navHtml = NAV_GROUPS.map(
    (g) => `
      <div>
        <div class="nav-group-label">${esc(g.label)}</div>
        <div class="nav-list">
          ${g.items
            .map(
              (it) => `<button type="button" class="nav-btn ${state.route === it.id ? 'active' : ''}" data-route="${esc(it.id)}">
                <span>${esc(it.label)}</span>
              </button>`,
            )
            .join('')}
        </div>
      </div>
    `,
  ).join('');

  root.innerHTML = `
    <div class="shell">
      <aside class="sidebar" id="fp-sidebar">
        <div class="sidebar-head">
          <a href="#overview" class="brand-wordmark" id="brand-home">
            <span class="brand-mark">FP</span>
            <span>FerrousPanel</span>
          </a>
        </div>
        <nav class="nav-scroll" aria-label="Điều hướng chính">${navHtml}</nav>
        <div class="sidebar-foot">
          <div class="sidebar-foot-meta">
            <span>${esc(state.user?.username || 'admin')}</span>
            <span>·</span>
            <span class="mono">:${esc(state.user?.panelPort || state.boot.panelPort || 8443)}</span>
          </div>
          <div class="sidebar-foot-meta">
            <span>Rust Native UI</span>
            <span>·</span>
            <span class="mono">v${esc(state.boot.version || '2.5.0')}</span>
          </div>
        </div>
      </aside>
      <div class="workspace">
        <header class="topbar">
          <div class="topbar-left">
            <button type="button" class="menu-toggle" id="mobile-menu-btn" aria-label="Mở menu">Menu</button>
            <div class="breadcrumb">
              <span class="breadcrumb-root">FerrousPanel</span>
              <span class="breadcrumb-sep">/</span>
              <span class="breadcrumb-curr" id="topbar-breadcrumb-curr">${esc(ROUTE_TITLES[state.route] || 'Tổng quan hệ thống')}</span>
            </div>
          </div>
          <div class="topbar-actions">
            <a class="btn btn-sm" href="/phpadmin/" target="_blank" rel="noopener">FerrousAdmin SQL</a>
            <button type="button" class="btn btn-sm" id="topbar-refresh-btn">Làm mới</button>
            <button type="button" class="btn btn-sm" id="theme-toggle-btn">${isDark ? 'Giao diện sáng' : 'Giao diện tối'}</button>
            <button type="button" class="btn btn-sm btn-danger" id="topbar-logout-btn">Đăng xuất</button>
          </div>
        </header>
        <main class="main-content" id="view-container"></main>
      </div>
    </div>
  `;

  document.getElementById('brand-home')?.addEventListener('click', (e) => {
    e.preventDefault();
    navigate('overview');
  });
  document.querySelectorAll('.nav-btn[data-route]').forEach((btn) => {
    btn.addEventListener('click', () => navigate(btn.dataset.route));
  });
  document.getElementById('mobile-menu-btn')?.addEventListener('click', () => {
    state.mobileNavOpen = !state.mobileNavOpen;
    updateShellChrome();
  });
  document.getElementById('topbar-refresh-btn')?.addEventListener('click', () => renderCurrentView());
  document.getElementById('theme-toggle-btn')?.addEventListener('click', toggleTheme);
  document.getElementById('topbar-logout-btn')?.addEventListener('click', handleLogout);

  renderCurrentView();
}

function renderLogin(errorMessage = '') {
  const root = document.getElementById('app');
  if (!root) return;
  const port = state.boot.panelPort || 8443;
  root.innerHTML = `
    <div class="login-shell">
      <div class="login-card">
        <div class="login-brand">
          <div class="brand-mark">FP</div>
          <div>
            <h1 class="login-title">FerrousPanel</h1>
            <p class="login-sub">Pure-Rust VPS Control Plane · Cổng <span class="mono">${esc(port)}</span> · Zero Node.js</p>
          </div>
        </div>
        ${errorMessage ? `<div class="alert-err" id="login-error">${esc(errorMessage)}</div>` : '<div id="login-error"></div>'}
        <form id="login-form">
          <div class="field">
            <label for="login-username">Tài khoản quản trị</label>
            <input class="input" id="login-username" name="username" autocomplete="username" placeholder="admin_ferrous" required />
          </div>
          <div class="field">
            <label for="login-password">Mật khẩu</label>
            <input class="input" id="login-password" name="password" type="password" autocomplete="current-password" required />
          </div>
          <div class="field">
            <label class="checkbox-row">
              <input type="checkbox" id="login-remember" />
              <span>Ghi nhớ phiên đăng nhập (30 ngày)</span>
            </label>
          </div>
          <button type="submit" class="btn btn-primary" style="width:100%" id="login-submit-btn">Đăng nhập</button>
        </form>
      </div>
    </div>
  `;

  const form = document.getElementById('login-form');
  form?.addEventListener('submit', async (e) => {
    e.preventDefault();
    const btn = document.getElementById('login-submit-btn');
    const errEl = document.getElementById('login-error');
    const username = document.getElementById('login-username')?.value.trim() || '';
    const password = document.getElementById('login-password')?.value || '';
    const rememberMe = Boolean(document.getElementById('login-remember')?.checked);

    if (btn) btn.disabled = true;
    if (errEl) {
      errEl.className = '';
      errEl.textContent = '';
    }
    try {
      const res = await api('POST', '/api/auth/login', { username, password, rememberMe });
      state.token = res.token || '';
      state.user = res.user || { username, panelPort: port };
      if (state.token) {
        if (rememberMe) {
          localStorage.setItem(TOKEN_KEY, state.token);
        } else {
          sessionStorage.setItem(TOKEN_KEY, state.token);
        }
      }
      toast('Đăng nhập thành công.');
      renderShell();
    } catch (err) {
      if (errEl) {
        errEl.className = 'alert-err';
        errEl.textContent = err.message || 'Sai tài khoản hoặc mật khẩu.';
      }
      if (btn) btn.disabled = false;
    }
  });
}

async function bootstrap() {
  applyTheme(localStorage.getItem(THEME_KEY) || 'dark');
  const hashRoute = window.location.hash.replace(/^#/, '');
  if (ROUTE_TITLES[hashRoute]) {
    state.route = hashRoute;
  }

  try {
    const sess = await api('GET', '/api/auth/session');
    if (sess && sess.authenticated) {
      state.user = sess.user || { username: 'admin_ferrous', panelPort: state.boot.panelPort };
      renderShell();
      return;
    }
  } catch (_) {
    // fall through to login screen
  }
  renderLogin();
}

window.addEventListener('hashchange', () => {
  if (!state.user) return;
  const h = window.location.hash.replace(/^#/, '');
  if (ROUTE_TITLES[h] && h !== state.route) {
    state.route = h;
    updateShellChrome();
    renderCurrentView();
  }
});

bootstrap();
