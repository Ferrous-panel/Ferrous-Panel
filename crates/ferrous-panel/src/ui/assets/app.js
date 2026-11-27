// FerrousPanel Pure-Rust Native Frontend Core (Zero Node.js)
import { renderOverview, renderApps, renderFiles, renderDatabases } from './views-core.js';
import { renderWebserver, renderPhpSoftware, renderCpanel, renderMail, renderEdgePolyglot } from './views-infra.js';
import { renderSecurity, renderOptimizer, renderSsh, renderSettingsJobs, renderSysops } from './views-security.js';

const TOKEN_KEY = 'fp_session_token';
const THEME_KEY = 'fp_theme';

export const state = window.__FP_STATE__ || (window.__FP_STATE__ = {
  boot: readBootConfig(),
  token: sessionStorage.getItem(TOKEN_KEY) || localStorage.getItem(TOKEN_KEY) || '',
  user: null,
  route: 'overview',
  mobileNavOpen: false,
  activeJobPoll: null,
  telemetryHistory: [],
  refreshTimer: null,
  capabilities: null,
  renderSeq: 0,
  viewAbortController: null,
});

export function isActiveRender(seq, expectedRoute = null) {
  if (seq !== state.renderSeq) return false;
  if (expectedRoute && state.route !== expectedRoute) return false;
  return true;
}

export function clearRefreshTimer() {
  if (state.refreshTimer) {
    clearTimeout(state.refreshTimer);
    clearInterval(state.refreshTimer);
    state.refreshTimer = null;
  }
}

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
  if (['running', 'active', 'online', 'completed', 'ok', 'healthy', 'installed', 'enabled', 'pass', 'ready', 'verified', 'connected', 'enforced'].includes(s)) return 'st-ok';
  if (['queued', 'pending', 'building', 'warning', 'warn', 'elevated', 'degraded', 'experimental', 'unverified', 'staged', 'standby', 'paused'].includes(s)) return 'st-warn';
  if (['failed', 'error', 'stopped', 'critical', 'under_attack', 'offline', 'missing', 'unavailable', 'not-installed', 'not_installed', 'engine_not_installed'].includes(s)) return 'st-err';
  return 'st-muted';
}

export async function loadCapabilities(force = false) {
  if (state.capabilities && !force) return state.capabilities;
  try {
    state.capabilities = await api('GET', '/api/capabilities');
  } catch (_) {
    state.capabilities = null;
  }
  return state.capabilities;
}

export const CAP_SOFTWARE_PKG_MAP = {
  edge_cloud_wasi: { pkgIds: ['pkg-wasmtime'], label: 'Cài Wasmtime (1-Click)' },
  edge_cloud_oci: { pkgIds: ['pkg-podman'], label: 'Cài Podman (1-Click)' },
  edge_cloud_wireguard: { pkgIds: ['pkg-wireguard'], label: 'Cài WireGuard (1-Click)' },
  mail_server: { pkgIds: ['pkg-postfix', 'pkg-dovecot', 'pkg-rspamd'], label: 'Cài Mail Stack (1-Click)' },
  fail2ban: { pkgIds: ['pkg-fail2ban'], label: 'Cài Fail2ban (1-Click)' },
  mariadb: { pkgIds: ['pkg-mariadb'], label: 'Cài MariaDB (1-Click)' },
  postgresql: { pkgIds: ['pkg-postgresql'], label: 'Cài PostgreSQL (1-Click)' },
  certbot: { pkgIds: ['pkg-certbot'], label: 'Cài Certbot (1-Click)' },
  nginx: { pkgIds: ['pkg-nginx'], label: 'Cài Nginx (1-Click)' },
};

export async function installCapabilityPackages(capIds) {
  const list = Array.isArray(capIds) ? capIds : [capIds];
  const pkgSet = new Set();
  for (const cid of list) {
    const entry = CAP_SOFTWARE_PKG_MAP[cid];
    if (entry && Array.isArray(entry.pkgIds)) {
      entry.pkgIds.forEach((p) => pkgSet.add(p));
    }
  }
  const pkgs = Array.from(pkgSet);
  if (!pkgs.length) return;
  for (const pkgId of pkgs) {
    await api('POST', `/api/software/${encodeURIComponent(pkgId)}/action`, { action: 'install' });
  }
  await loadCapabilities(true);
  await renderCurrentView(true);
}

export function capOf(id) {
  const list = state.capabilities?.capabilities || state.capabilities?.features || [];
  return list.find((c) => c.id === id) || null;
}

export function isCapUnavailable(id) {
  const c = capOf(id);
  return Boolean(c && c.status === 'unavailable');
}

export function capBannerHtml(ids) {
  const list = (Array.isArray(ids) ? ids : [ids])
    .map((id) => capOf(id))
    .filter((c) => c && c.status !== 'ready');
  if (!list.length) return '';
  return list
    .map((c) => {
      const installMeta = (c.status === 'unavailable' || c.status === 'degraded') ? CAP_SOFTWARE_PKG_MAP[c.id] : null;
      return `
      <div class="cap-banner ${esc(c.status)}">
        <div class="cap-banner-head">
          <strong>${esc(c.label || c.name || c.id)}</strong>
          <div class="row" style="gap:8px">
            ${installMeta ? `<button type="button" class="btn btn-sm btn-primary" data-cap-install="${esc(c.id)}">${esc(installMeta.label)}</button>` : ''}
            <span class="${statusClass(c.status)}">${esc(String(c.status || '').toUpperCase())}</span>
          </div>
        </div>
        <div>${esc(c.reason || 'Tính năng chưa sẵn sàng trên máy chủ này.')}</div>
        ${c.fixHint ? `<div class="cap-hint">Gợi ý khắc phục: ${esc(c.fixHint)}</div>` : ''}
      </div>
    `;
    })
    .join('');
}

export function capDisabledAttr(id) {
  const c = capOf(id);
  if (!c || c.status !== 'unavailable') return '';
  const title = `${c.reason || 'Không khả dụng'}${c.fixHint ? ` — ${c.fixHint}` : ''}`;
  return `disabled title="${esc(title)}"`;
}

export async function withBusy(btn, fn) {
  if (!btn) return fn();
  if (btn.disabled) return;
  const origText = btn.textContent;
  btn.disabled = true;
  btn.dataset.busy = '1';
  if (origText && origText.length < 32) {
    btn.textContent = 'Đang xử lý…';
  }
  try {
    return await fn();
  } finally {
    btn.dataset.busy = '';
    if (document.body.contains(btn)) {
      btn.disabled = false;
      if (origText && origText.length < 32) {
        btn.textContent = origText;
      }
    }
  }
}

export async function waitForPanelReconnect(maxWaitMs = 12000, stepMs = 650) {
  const deadline = Date.now() + maxWaitMs;
  while (Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, stepMs));
    try {
      const resp = await fetch('/api/health', { cache: 'no-store', credentials: 'same-origin' });
      if (resp.ok) return true;
    } catch (_) {
      // still restarting
    }
  }
  return false;
}

function formatHttpError(status, contentType, text, data) {
  if (data && typeof data === 'object') {
    const raw = data.error || data.message;
    if (typeof raw === 'string' && raw.trim()) {
      const clean = raw.trim();
      return clean.length > 340 ? `${clean.slice(0, 340)}…` : clean;
    }
  }

  const ct = String(contentType || '').toLowerCase();
  const body = String(text || '').trim();
  const looksHtml =
    ct.includes('text/html') ||
    /^\s*(<!doctype\s+html|<html\b|<head\b|<body\b)/i.test(body);

  if (looksHtml) {
    const titleMatch = body.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
    const title = titleMatch ? titleMatch[1].replace(/\s+/g, ' ').trim() : '';
    const rayMatch = body.match(/Ray ID:\s*<[^>]*>([a-zA-Z0-9-]+)</i) || body.match(/Ray ID:\s*([a-zA-Z0-9-]+)/i);
    const raySuffix = rayMatch ? ` (Ray ID: ${rayMatch[1]})` : '';
    if (status === 502) {
      return `Lỗi cổng kết nối HTTP 502 Bad Gateway${raySuffix}. Máy chủ hoặc dịch vụ backend chưa phản hồi — vui lòng kiểm tra trạng thái dịch vụ và thử lại.`;
    }
    if (status === 503) {
      return `Dịch vụ tạm thời gián đoạn (HTTP 503)${raySuffix}. Vui lòng thử lại sau ít giây.`;
    }
    if (status === 504 || status === 524) {
      return `Quá thời gian chờ phản hồi từ máy chủ (HTTP ${status})${raySuffix}. Tác vụ có thể đang chạy chậm hoặc bị ngắt bởi proxy.`;
    }
    if (title) {
      return `HTTP ${status}: ${title}${raySuffix}`;
    }
    return `Máy chủ hoặc proxy trả về trang lỗi HTML (HTTP ${status})${raySuffix}.`;
  }

  if (body) {
    const singleLine = body.replace(/\s+/g, ' ').trim();
    return singleLine.length > 300 ? `${singleLine.slice(0, 300)}…` : singleLine;
  }
  return `Yêu cầu thất bại (HTTP ${status})`;
}

export function toast(message, kind = 'ok') {
  const root = document.getElementById('toast-root');
  if (!root) return;
  while (root.children.length >= 3) {
    root.firstElementChild?.remove();
  }
  const raw = String(message ?? '').trim();
  const display = raw.length > 320 ? `${raw.slice(0, 320)}…` : raw;
  const el = document.createElement('div');
  el.className = `toast ${kind === 'err' ? 'err' : 'ok'}`;
  el.setAttribute('role', kind === 'err' ? 'alert' : 'status');
  el.innerHTML = `<div class="toast-msg">${esc(display)}</div><button type="button" class="toast-close" aria-label="Đóng thông báo">×</button>`;
  const removeToast = () => {
    el.remove();
  };
  el.querySelector('.toast-close')?.addEventListener('click', removeToast);
  root.appendChild(el);
  setTimeout(removeToast, kind === 'err' ? 7500 : 4800);
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

const getApiCache = window.__FP_API_CACHE__ || (window.__FP_API_CACHE__ = new Map());
const inflightGetMap = window.__FP_INFLIGHT_MAP__ || (window.__FP_INFLIGHT_MAP__ = new Map());
const visitedRoutes = window.__FP_VISITED_ROUTES__ || (window.__FP_VISITED_ROUTES__ = new Set());
const GET_CACHE_TTL_MS = 15000;

export function invalidateApiCache() {
  getApiCache.clear();
}

function isCacheableGetPath(path, responseType, noCache) {
  if (noCache || responseType !== 'json') return false;
  if (
    path.includes('/poll') ||
    path.includes('/logs') ||
    path.includes('download=1') ||
    path.includes('refresh=1') ||
    path.startsWith('/api/auth/')
  ) {
    return false;
  }
  return true;
}

function cloneCachedValue(val) {
  if (val === null || typeof val !== 'object') return val;
  if (typeof structuredClone === 'function') {
    try { return structuredClone(val); } catch (_) {}
  }
  return JSON.parse(JSON.stringify(val));
}

export async function api(
  method,
  path,
  body = undefined,
  {
    rawBody = false,
    contentType = 'application/json',
    responseType = 'json',
    timeoutMs = 35000,
    signal = null,
    keepAlive = false,
    retries = undefined,
    noCache = false,
  } = {},
) {
  const upperMethod = String(method || 'GET').toUpperCase();
  const canCache = upperMethod === 'GET' && isCacheableGetPath(path, responseType, noCache);

  if (canCache) {
    const hit = getApiCache.get(path);
    if (hit && Date.now() - hit.ts < GET_CACHE_TTL_MS) {
      return cloneCachedValue(hit.data);
    }
    if (!signal && inflightGetMap.has(path)) {
      try {
        const shared = await inflightGetMap.get(path);
        return cloneCachedValue(shared);
      } catch (_) {
        inflightGetMap.delete(path);
      }
    }
  }

  const runFetch = async () => {
    const maxRetries = typeof retries === 'number'
      ? retries
      : (upperMethod === 'GET' && !path.includes('/poll') ? 1 : 0);

    let attempt = 0;
    while (true) {
      const headers = {
        'Accept': responseType === 'blob' ? '*/*' : 'application/json',
        'X-Requested-With': 'FerrousPanel-UI',
      };
      if (state.token) {
        headers['Authorization'] = `Bearer ${state.token}`;
        headers['X-FP-Session'] = state.token;
      }

      const reqCtrl = new AbortController();
      let timedOut = false;
      const viewSignal = null;

      const onExternalAbort = () => reqCtrl.abort('cancelled');
      if (signal) {
        if (signal.aborted) {
          const abortErr = new Error('Yêu cầu đã được hủy do chuyển trang.');
          abortErr.isAbort = true;
          throw abortErr;
        }
        signal.addEventListener('abort', onExternalAbort, { once: true });
      }
      if (viewSignal) {
        if (viewSignal.aborted) {
          const abortErr = new Error('Yêu cầu đã được hủy do chuyển trang.');
          abortErr.isAbort = true;
          throw abortErr;
        }
        viewSignal.addEventListener('abort', onExternalAbort, { once: true });
      }

      const effectiveTimeout = Number(timeoutMs) > 0 ? Number(timeoutMs) : 35000;
      const timeoutHandle = setTimeout(() => {
        timedOut = true;
        reqCtrl.abort('timeout');
      }, effectiveTimeout);

      const init = {
        method: upperMethod,
        credentials: 'same-origin',
        headers,
        signal: reqCtrl.signal,
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

      let resp;
      try {
        resp = await fetch(path, init);
      } catch (fetchErr) {
        clearTimeout(timeoutHandle);
        if (signal) signal.removeEventListener('abort', onExternalAbort);
        if (viewSignal) viewSignal.removeEventListener('abort', onExternalAbort);

        if (timedOut) {
          const err = new Error(`Quá thời gian chờ phản hồi từ máy chủ (${Math.round(effectiveTimeout / 1000)}s).`);
          err.isTimeout = true;
          throw err;
        }
        if (reqCtrl.signal.aborted || fetchErr?.name === 'AbortError') {
          const abortErr = new Error('Yêu cầu đã được hủy do chuyển trang.');
          abortErr.isAbort = true;
          throw abortErr;
        }
        if (attempt < maxRetries) {
          attempt += 1;
          await new Promise((r) => setTimeout(r, 350 * attempt));
          continue;
        }
        throw new Error(`Lỗi kết nối mạng tới máy chủ: ${fetchErr?.message || 'NetworkError'}`);
      } finally {
        clearTimeout(timeoutHandle);
        if (signal) signal.removeEventListener('abort', onExternalAbort);
        if (viewSignal) viewSignal.removeEventListener('abort', onExternalAbort);
      }

      if (resp.status === 401 && path !== '/api/auth/login') {
        clearRefreshTimer();
        getApiCache.clear();
        visitedRoutes.clear();
        state.renderSeq += 1;
        if (state.viewAbortController) {
          state.viewAbortController.abort('unauthorized');
          state.viewAbortController = null;
        }
        state.user = null;
        state.token = '';
        sessionStorage.removeItem(TOKEN_KEY);
        localStorage.removeItem(TOKEN_KEY);
        renderLogin('Phiên đăng nhập đã hết hạn. Vui lòng đăng nhập lại.');
        throw new Error('Unauthorized');
      }

      if (attempt < maxRetries && [502, 503, 504].includes(resp.status)) {
        attempt += 1;
        await new Promise((r) => setTimeout(r, 450 * attempt));
        continue;
      }

      const respContentType = resp.headers.get('content-type') || '';

      if (responseType === 'blob') {
        if (!resp.ok) {
          const txt = await resp.text();
          const err = new Error(formatHttpError(resp.status, respContentType, txt, null));
          err.status = resp.status;
          throw err;
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
        const msg = formatHttpError(resp.status, respContentType, text, data);
        const err = new Error(msg);
        err.status = resp.status;
        throw err;
      }
      if (upperMethod !== 'GET') {
        getApiCache.clear();
      } else if (canCache) {
        if (getApiCache.size > 120) getApiCache.clear();
        getApiCache.set(path, { ts: Date.now(), data: cloneCachedValue(data) });
      }
      return data;
    }
  };

  if (canCache && !signal) {
    const p = runFetch().finally(() => {
      inflightGetMap.delete(path);
    });
    inflightGetMap.set(path, p);
    const out = await p;
    return cloneCachedValue(out);
  }
  return runFetch();
}

function applyTheme(theme) {
  const isDark = theme !== 'light';
  document.documentElement.classList.toggle('dark', isDark);
  localStorage.setItem(THEME_KEY, isDark ? 'dark' : 'light');
}

function toggleTheme() {
  const next = document.documentElement.classList.contains('dark') ? 'light' : 'dark';
  applyTheme(next);
  const label = next === 'dark' ? 'Giao diện sáng' : 'Giao diện tối';
  const btn = document.getElementById('theme-toggle-btn');
  if (btn) btn.textContent = label;
  const mobBtn = document.getElementById('sidebar-theme-btn');
  if (mobBtn) mobBtn.textContent = label;
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
      { id: 'edge', label: 'Edge Cloud & Polyglot' },
    ],
  },
  {
    label: 'Bảo mật & Hệ thống',
    items: [
      { id: 'sysops', label: 'Tiến trình & Hệ thống' },
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
  try {
    window.scrollTo({ top: 0, left: 0, behavior: 'instant' });
  } catch (_) {
    window.scrollTo(0, 0);
  }
  if (document.documentElement) document.documentElement.scrollTop = 0;
  if (document.body) document.body.scrollTop = 0;
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
  const backdrop = document.getElementById('fp-sidebar-backdrop');
  if (backdrop) backdrop.classList.toggle('open', state.mobileNavOpen);
  document.body.classList.toggle('nav-open', state.mobileNavOpen);
}

function renderSkeleton(container) {
  container.innerHTML = `
    <div style="display:flex;flex-direction:column;gap:16px" aria-busy="true" aria-label="Đang tải dữ liệu">
      <div style="display:flex;flex-direction:column;gap:8px">
        <div class="skel skel-title"></div>
        <div class="skel skel-sub"></div>
      </div>
      <div class="kpi-grid">
        <div class="skel skel-kpi"></div>
        <div class="skel skel-kpi"></div>
        <div class="skel skel-kpi"></div>
        <div class="skel skel-kpi"></div>
      </div>
      <div class="skel skel-table"></div>
    </div>
  `;
}

export function openQuickJump() {
  const allItems = NAV_GROUPS.flatMap((g) => g.items.map((it) => ({ ...it, group: g.label })));
  openModal('Chuyển nhanh phân hệ (Ctrl+K)', `
    <input class="input" id="cmdk-input" placeholder="Gõ tên phân hệ (Website, SQL, PHP, Mail, Security, Edge…)" autocomplete="off" />
    <div class="cmdk-list" id="cmdk-list"></div>
  `, {
    onMount(body) {
      const input = body.querySelector('#cmdk-input');
      const listEl = body.querySelector('#cmdk-list');
      let selectedIdx = 0;
      let filtered = allItems;

      function drawList() {
        const q = (input?.value || '').trim().toLowerCase();
        filtered = q
          ? allItems.filter((it) => it.label.toLowerCase().includes(q) || it.id.includes(q) || it.group.toLowerCase().includes(q))
          : allItems;
        if (selectedIdx >= filtered.length) selectedIdx = 0;
        listEl.innerHTML = filtered.length
          ? filtered.map((it, idx) => `
              <button type="button" class="cmdk-item ${idx === selectedIdx ? 'active' : ''}" data-cmdk-route="${esc(it.id)}">
                <span><strong>${esc(it.label)}</strong></span>
                <span class="cmdk-group">${esc(it.group)}</span>
              </button>`).join('')
          : '<div class="empty-state" style="padding:16px">Không tìm thấy phân hệ phù hợp.</div>';
        listEl.querySelectorAll('[data-cmdk-route]').forEach((b) => {
          b.addEventListener('click', () => {
            closeModal();
            navigate(b.dataset.cmdkRoute);
          });
        });
      }

      input?.addEventListener('input', () => { selectedIdx = 0; drawList(); });
      input?.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          if (filtered.length) { selectedIdx = (selectedIdx + 1) % filtered.length; drawList(); }
        } else if (e.key === 'ArrowUp') {
          e.preventDefault();
          if (filtered.length) { selectedIdx = (selectedIdx - 1 + filtered.length) % filtered.length; drawList(); }
        } else if (e.key === 'Enter' && filtered[selectedIdx]) {
          e.preventDefault();
          closeModal();
          navigate(filtered[selectedIdx].id);
        }
      });
      drawList();
      setTimeout(() => input?.focus(), 20);
    },
  });
}

export async function renderCurrentView(forceCaps = false) {
  clearRefreshTimer();
  if (state.viewAbortController) {
    state.viewAbortController.abort('route-change');
  }
  state.viewAbortController = new AbortController();
  const seq = ++state.renderSeq;
  const routeAtStart = state.route;

  const container = document.getElementById('view-container');
  if (!container) return;
  if (!visitedRoutes.has(routeAtStart) || !container.children.length) {
    renderSkeleton(container);
  }

  try {
    await loadCapabilities(forceCaps);
    if (!isActiveRender(seq, routeAtStart)) return;

    switch (routeAtStart) {
      case 'overview':
        await renderOverview(container, seq);
        break;
      case 'apps':
        await renderApps(container, seq);
        break;
      case 'files':
        await renderFiles(container, seq);
        break;
      case 'databases':
        await renderDatabases(container, seq);
        break;
      case 'webserver':
        await renderWebserver(container, seq);
        break;
      case 'software':
        await renderPhpSoftware(container, seq);
        break;
      case 'cpanel':
        await renderCpanel(container, seq);
        break;
      case 'mail':
        await renderMail(container, seq);
        break;
      case 'edge':
        await renderEdgePolyglot(container, seq);
        break;
      case 'sysops':
        await renderSysops(container, seq);
        break;
      case 'security':
        await renderSecurity(container, seq);
        break;
      case 'optimizer':
        await renderOptimizer(container, seq);
        break;
      case 'ssh':
        await renderSsh(container, seq);
        break;
      case 'settings':
        await renderSettingsJobs(container, seq);
        break;
      default:
        await renderOverview(container, seq);
    }
    if (isActiveRender(seq, routeAtStart)) {
      visitedRoutes.add(routeAtStart);
    }
  } catch (err) {
    if (!isActiveRender(seq, routeAtStart)) return;
    if (err?.isAbort) return;
    if (err.message !== 'Unauthorized') {
      container.innerHTML = `
        <div class="surface">
          <div class="alert-err">Không thể tải phân hệ: ${esc(err.message)}</div>
          <button type="button" class="btn" id="retry-view-btn">Thử tải lại</button>
        </div>
      `;
      document.getElementById('retry-view-btn')?.addEventListener('click', () => renderCurrentView(true));
    }
  }
}

export async function handleLogout() {
  clearRefreshTimer();
  state.renderSeq += 1;
  if (state.viewAbortController) {
    state.viewAbortController.abort('logout');
    state.viewAbortController = null;
  }
  try {
    await api('POST', '/api/auth/logout', { token: state.token || null }, { keepAlive: true, timeoutMs: 8000 });
  } catch (_) {
    // clear local session regardless
  }
  state.token = '';
  state.user = null;
  state.capabilities = null;
  sessionStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(TOKEN_KEY);
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
          <button type="button" class="sidebar-close-btn" id="mobile-nav-close" aria-label="Đóng menu">×</button>
        </div>
        <nav class="nav-scroll" aria-label="Điều hướng chính">${navHtml}</nav>
        <div class="sidebar-foot">
          <div class="sidebar-mobile-actions">
            <a class="btn btn-sm" href="/phpadmin/${state.token ? `?fp_sso=${encodeURIComponent(state.token)}` : ''}" target="_blank" rel="noopener">FerrousAdmin SQL</a>
            <button type="button" class="btn btn-sm" id="sidebar-theme-btn">${isDark ? 'Giao diện sáng' : 'Giao diện tối'}</button>
          </div>
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
      <div class="sidebar-backdrop" id="fp-sidebar-backdrop"></div>
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
            <button type="button" class="btn btn-sm hide-mobile" id="topbar-cmdk-btn" title="Chuyển nhanh phân hệ">Chuyển nhanh <span class="kbd-hint">Ctrl+K</span></button>
            <a class="btn btn-sm hide-mobile" href="/phpadmin/${state.token ? `?fp_sso=${encodeURIComponent(state.token)}` : ''}" target="_blank" rel="noopener">FerrousAdmin SQL</a>
            <button type="button" class="btn btn-sm" id="topbar-refresh-btn">Làm mới</button>
            <button type="button" class="btn btn-sm hide-mobile" id="theme-toggle-btn">${isDark ? 'Giao diện sáng' : 'Giao diện tối'}</button>
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
  document.getElementById('mobile-nav-close')?.addEventListener('click', () => {
    state.mobileNavOpen = false;
    updateShellChrome();
  });
  document.getElementById('fp-sidebar-backdrop')?.addEventListener('click', () => {
    state.mobileNavOpen = false;
    updateShellChrome();
  });
  document.getElementById('topbar-cmdk-btn')?.addEventListener('click', openQuickJump);
  document.getElementById('topbar-refresh-btn')?.addEventListener('click', () => renderCurrentView(true));
  document.getElementById('theme-toggle-btn')?.addEventListener('click', toggleTheme);
  document.getElementById('sidebar-theme-btn')?.addEventListener('click', toggleTheme);
  document.getElementById('topbar-logout-btn')?.addEventListener('click', handleLogout);

  renderCurrentView(true);
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
            <p class="login-sub">VPS Control Plane · Cổng <span class="mono">${esc(port)}</span></p>
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

if (!window.__FP_BOOTSTRAPPED__) {
  window.__FP_BOOTSTRAPPED__ = true;

  document.addEventListener('click', (e) => {
    const btn = e.target?.closest?.('[data-cap-install]');
    if (!btn) return;
    const capId = btn.dataset.capInstall;
    if (!capId) return;
    withBusy(btn, async () => {
      try {
        toast('Đang tự động cài đặt gói dịch vụ hệ thống (1-Click)…');
        await installCapabilityPackages(capId);
        toast('Đã cài đặt và kích hoạt tính năng thành công.');
      } catch (err) {
        toast(err.message || 'Không thể cài đặt tự động gói dịch vụ.', 'err');
      }
    });
  });

  window.addEventListener('hashchange', () => {
    if (!state.user) return;
    const h = window.location.hash.replace(/^#/, '');
    if (ROUTE_TITLES[h] && h !== state.route) {
      state.route = h;
      updateShellChrome();
      try {
        window.scrollTo({ top: 0, left: 0, behavior: 'instant' });
      } catch (_) {
        window.scrollTo(0, 0);
      }
      renderCurrentView();
    }
  });

  window.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
      if (state.user) {
        e.preventDefault();
        openQuickJump();
      }
      return;
    }
    if (e.key !== 'Escape') return;
    if (state.mobileNavOpen) {
      state.mobileNavOpen = false;
      updateShellChrome();
    }
    const modalRoot = document.getElementById('modal-root');
    if (modalRoot && modalRoot.innerHTML) closeModal();
    const drawerRoot = document.getElementById('drawer-root');
    if (drawerRoot && drawerRoot.innerHTML) closeJobDrawer();
  });

  bootstrap();
}
